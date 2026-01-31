// @ts-nocheck
const LONG_RITE_TOKEN = "long-rite";
const taskState: Record<string, string> = {};

function extractText(params: any): string {
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

function newMessage(messageId: string, text: string) {
  return {
    messageId,
    role: "ROLE_AGENT",
    parts: [
      {
        text
      }
    ]
  };
}

function newTask(taskId: string, contextId: string, message: any) {
  taskState[taskId] = "TASK_STATE_WORKING";
  return {
    id: taskId,
    contextId,
    status: {
      state: "TASK_STATE_WORKING"
    },
    history: message ? [message] : []
  };
}

async function fakeStreamRiteTool(text: string, taskId: string, contextId: string) {
  if (taskState[taskId] === "TASK_STATE_CANCELED") {
    return [
      {
        statusUpdate: {
          taskId,
          contextId,
          status: {
            state: "TASK_STATE_CANCELED",
            message: newMessage("rite-canceled", `Rite canceled: ${text}`)
          }
        }
      }
    ];
  }
  const statusUpdate = {
    statusUpdate: {
      taskId,
      contextId,
      status: {
        state: "TASK_STATE_WORKING",
        message: newMessage("rite-status", `Rite underway: ${text}`)
      }
    }
  };
  const artifactUpdate = {
    artifactUpdate: {
      taskId,
      contextId,
      append: false,
      lastChunk: true,
      artifact: {
        artifactId: "rite-log-001",
        name: "Rite Log",
        description: "Compiled litany fragments",
        parts: [
          {
            mediaType: "application/json",
            data: {
              step: "seal",
              omen: "frost on the reactor glyphs",
              note: "recite canticle XVII"
            }
          }
        ]
      }
    }
  };
  const message = {
    message: newMessage("rite-msg-001", `Rite complete: ${text}`)
  };
  return [statusUpdate, artifactUpdate, message];
}


async function handle_a2a_request(request: any) {
  const method = request && request.method;
  const params = request && request.params ? request.params : {};
  const text = extractText(params);
  const messageId = params.message && params.message.messageId ? params.message.messageId : "msg-void-001";
  const contextId = params.message && params.message.contextId ? params.message.contextId : "ctx-void-001";
  const taskId = `rite-task-${messageId}`;

  if (method === "message.send") {
    if (text.includes(LONG_RITE_TOKEN)) {
      return {
        task: newTask(taskId, contextId, params.message)
      };
    }
    // Use BAML to determine if a tool call is needed
    try {
      const toolChoice = await ChooseRiteTool({ user_message: text });
      const toolResult = await __tool_from_baml_result(JSON.stringify(toolChoice));
      return {
        message: newMessage(
          `resp-${messageId}`,
          `BAML rite complete: sum=${toolResult.result}`
        ),
        task: newTask(taskId, contextId, params.message)
      };
    } catch (e) {
      // If tool calling fails or isn't needed, continue with normal response
    }
    return {
      message: newMessage(`resp-${messageId}`, `Blessings upon ${text}`)
    };
  }

  if (method === "message.sendStream") {
    return await fakeStreamRiteTool(text, taskId, contextId);
  }

  return {
    message: newMessage(`resp-${messageId}`, `Unknown rite for ${text}`)
  };
}

async function handle_a2a_cancel(args: any) {
  const taskId = args && args.id ? args.id : "unknown";
  taskState[taskId] = "TASK_STATE_CANCELED";
  return {
    statusUpdate: {
      taskId,
      status: {
        state: "TASK_STATE_CANCELED",
        message: newMessage("rite-cancel", `Cancellation rites accepted for ${taskId}`)
      }
    }
  };
}

globalThis.handle_a2a_request = handle_a2a_request;
globalThis.handle_a2a_cancel = handle_a2a_cancel;
