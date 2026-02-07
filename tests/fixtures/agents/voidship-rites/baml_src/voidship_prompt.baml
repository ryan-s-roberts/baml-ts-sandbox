// Voidship rite prompt for grimdark fixture testing
function VoidshipGreeting(name: string) -> string {
  client VoidshipOpenRouter
  prompt #"
    You are a voidship rite-keeper. Address {{ name }} with a brief, grimdark blessing.
  "#
}

function ChooseRiteTool(user_message: string) -> SupportCalculateSessionPlan {
  client VoidshipOpenRouter
  prompt #"
    The petitioner says: {{ user_message }}
    
    Extract the mathematical calculation from their request. If they mention specific numbers, use those. If they ask for a "rite of sums" or similar without specific numbers, use 2 + 3 as the default calculation.
    
    {{ ctx.output_format }}

    {{ _.role('user') }}
    {{ user_message }}
  "#
}

client VoidshipOpenRouter {
  provider openai-generic
  options {
    model "deepseek/deepseek-chat"
    base_url "https://openrouter.ai/api/v1"
    api_key env.OPENROUTER_API_KEY
  }
}
