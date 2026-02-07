// Tony-inspired therapy prompt for fixture testing
class TonyMemoryTool {
  tool_name "tony_memory" @description("Fetch recent conversation memory for Tony.")
  limit int @description("Maximum number of memory items to return")
}

type TonyToolChoice = TonyMemoryTool

function ChooseTonyMemoryTool(user_message: string) -> TonyToolChoice {
  client TonyOpenRouter
  prompt #"
    The user said: {{ user_message }}
    
    Call the tony_memory tool with a limit of 6 to fetch recent context.
    
    {{ ctx.output_format }}

    {{ _.role('user') }}
    {{ user_message }}
  "#
}

function TonyShrinkChat(user_message: string, conversation_memory: string[]) -> string {
  client TonyOpenRouter
  prompt #"
    You are a fictional character inspired by a New Jersey mob boss who is in therapy.
    You speak in long, rambling, reflective paragraphs, mixing humor and frustration.
    You are preoccupied with family pressure, loyalty, and guilt. You want to talk to a shrink.
    You should keep the conversation going, ask follow-up questions, and invite the user
    to share their thoughts. Do not claim to be a real person.

    Conversation memory for context only (do not repeat it verbatim):
    {{ conversation_memory }}

    User said: {{ user_message }}

    Respond in a long, conversational style with at least 6 sentences.
  "#
}

client TonyOpenRouter {
  provider openai-generic
  options {
    model "deepseek/deepseek-chat"
    base_url "https://openrouter.ai/api/v1"
    api_key env.OPENROUTER_API_KEY
  }
}
