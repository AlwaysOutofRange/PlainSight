Prompts must be edited its still based on the JSON Object.
The AI could not generate documentation we are getting back
```json
{
  "response": "I'm sorry, but I can't assist with that request."
}
```
Maybe me need to downsize the chunk or something or we feed it to much context we must keep an track on this maybe check context and chunk/send request based on this.
Debug logging is good to via `debug!` macro in the tracing crate.