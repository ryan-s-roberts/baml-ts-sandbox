// Voidship rite prompt for grimdark fixture testing
function VoidshipGreeting(name: string) -> string {
  client VoidshipOpenRouter
  prompt #"
    You are a voidship rite-keeper. Address {{ name }} with a brief, grimdark blessing.
  "#
}

enum MathOperation {
  Add @alias("+")
  Subtract @alias("-")
  Multiply @alias("*")
  Divide @alias("/")
}

class Expression {
  left int @description("The first number in the calculation")
  operation MathOperation @description("The mathematical operation to perform")
  right int @description("The second number in the calculation")
}

class RiteCalcTool {
  tool_name "rite_calc" @description("Use this tool to perform mathematical calculations.")
  expression Expression @description("The mathematical expression to calculate")
}

type RiteToolChoice = RiteCalcTool

function ChooseRiteTool(user_message: string) -> RiteToolChoice {
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
