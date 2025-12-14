// Simple test prompt for BAML compilation
function SimpleGreeting(name: string) -> string {
  client "openai/gpt-4o-mini"
  prompt #"
    Say hello to {{ name }} in a friendly way.
  "#
}

