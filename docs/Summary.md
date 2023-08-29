### **Voracious: An Interactive GPT Chat Application**

#### **Summary**:

The "Voracious" application serves as an interactive chat interface with OpenAI's GPT model. Users can initiate chat sessions, send messages, and receive responses from GPT in real-time. The application provides a command-line interface for interactions and supports session management capabilities.

- **GPT Integration**: Users communicate with the GPT model via the application. The integration is facilitated through the `async_openai` library.
  
- **Chat Sessions**: Users can initiate new chat sessions or continue from previously stored sessions. All chat sessions get saved in individual files, differentiated by timestamps. By default, the application picks up the last chat session.

- **User Interface**: The application offers a command-line interface. Messages from GPT are color-coded for differentiation. Users can exit the chat by typing specific commands or using keyboard shortcuts.

- **Logging**: All interactions, including user messages and GPT's responses, are logged. Logs are organized by date, stored in a dedicated directory.

- **Error Handling**: The application is built to handle potential errors, providing users with informative error messages when issues arise.
