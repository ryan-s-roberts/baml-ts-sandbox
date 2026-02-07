// @ts-nocheck
const conversationMemory = {};
function extractText(params) {
  if (!params) {
    return "unknown";
  }
  if (typeof params.text === "string") {
    return params.text;
  }
  const message = params.message;
  if (message && Array.isArray(message.parts) && message.parts.length > 0) {
    const first = message.parts[0];
    if (first && typeof first.text === "string") {
      return first.text;
    }
  }
  return "unknown";
}
function newMessage(messageId, text) {
  return {
    messageId,
    role: "ROLE_AGENT",
    parts: [{ text }],
  };
}
function addMemory(contextId, text) {
  if (!conversationMemory[contextId]) {
    conversationMemory[contextId] = [];
  }
  conversationMemory[contextId].push(text);
  if (conversationMemory[contextId].length > 6) {
    conversationMemory[contextId].shift();
  }
}
async function tony_memory(args) {
  const contextId = globalThis.__baml_context_id || "ctx-tony-001";
  const limit =
    args && typeof args.limit === "number" && Number.isFinite(args.limit)
      ? Math.max(1, Math.min(20, Math.floor(args.limit)))
      : 6;
  const memory = conversationMemory[contextId] || [];
  return {
    context_id: contextId,
    memory: memory.slice(-limit),
  };
}
async function buildBamlResponse(text, contextId) {
  try {
    globalThis.__baml_context_id = contextId;
    const toolChoice = await ChooseTonyMemoryTool({ user_message: text });
    const toolName =
      toolChoice && typeof toolChoice.tool_name === "string"
        ? toolChoice.tool_name
        : "tony_memory";
    const toolArgs = {
      limit:
        toolChoice && typeof toolChoice.limit === "number" ? toolChoice.limit : 6,
    };
    const toolResult = await invokeTool(toolName, toolArgs);
    const memory = toolResult && Array.isArray(toolResult.memory) ? toolResult.memory : [];
    return await TonyShrinkChat({
      user_message: text,
      conversation_memory: memory,
    });
  } catch (err) {
    return "Alright, I got nothin'. Try sayin' that again.";
  }
}
async function handle_a2a_request(request) {
  const method = request && request.method;
  const params = request && request.params ? request.params : {};
  const text = extractText(params);
  const messageId = params.message.messageId;
  const contextId = params.message.contextId;
  if (method === "message.send" || method === "message.sendStream") {
    addMemory(contextId, text);
    return { message: newMessage(`resp-${messageId}`, await buildBamlResponse(text, contextId)) };
  }
  return { message: newMessage(`resp-${messageId}`, `I don't know what to do with "${text}".`) };
}
globalThis.handle_a2a_request = handle_a2a_request;
globalThis.tony_memory = tony_memory;
