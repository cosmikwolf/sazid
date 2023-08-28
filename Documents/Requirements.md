### **Voracious: An Interactive GPT Chat Application**

#### **Functional Requirements**:

1. **GPT Integration**:
    - Utilize the `async_openai` library to integrate with OpenAI's GPT model.
    - Establish a connection to the GPT API using provided API keys.
    - Send user messages to the GPT model and retrieve generated responses.

2. **Chat Session Management**:
    - Provide command-line options to:
        - Start a new chat session.
        - Continue a previously stored session.
    - Save chat sessions in individual files named with the format: `session-YYYY-MM-DD_HH-MM.json`.
    - Automatically pick the latest session if no specific session is mentioned by the user.
    - Store the last-used session's filename in a text file named `last_session.txt`.

3. **User Interaction**:
    - Implement a command-line interface for the user to interact with the application.
    - Accept user input for messages to be sent to GPT.
    - Allow session exit through any of the following commands: "exit", "quit", or using the keyboard shortcut `Ctrl+C`.
    - Display a message indicating graceful exit upon termination.

4. **Message Display**:
    - Display messages on the command-line interface with clear distinction between user and GPT messages.
    - Color-code GPT messages for easy differentiation.
    - Label messages from previous sessions with the prefix "(from previous session)".

5. **Logging**:
    - Record all chat interactions, including user input and GPT responses.
    - Organize logs by date and store them in a designated directory named `logs`.
    - Save chat messages in a history file named `history.txt` for maintaining chat history across sessions.

6. **Error Handling**:
    - Implement error handling mechanisms to manage potential issues related to:
        - GPT API connection failures.
        - File read/write operations.
        - Session management.
    - Display user-friendly error messages to inform the user about the nature of the encountered error.

7. **Command-line Interface**:
    - Implement command-line arguments for enhanced session management:
        - `-n` or `--new`: Start a new chat session.
        - `-c` or `--continue`: Continue from a specified session file.
    - Display version, author, and other metadata information for the application when queried with appropriate command-line arguments.

8. **Data Serialization and Storage**:
    - Serialize chat messages in JSON format for storing in session files.
    - Ensure that each message stored consists of:
        - Role (User or Assistant).
        - Content of the message.
    - Deserialize JSON data when loading from session files.

9. **Timestamp Management**:
    - Generate timestamps based on the system's local time zone settings.
    - Use timestamps for naming session files and recording chat interactions.

10. **Modularity and Code Structure**:
    - Separate the main functionalities into distinct modules:
        - GPT Integration: Handle all interactions with the GPT model.
        - Session Management: Manage saving, loading, and continuation of chat sessions.
        - User Interface: Manage user input, message display, and command-line interactions.
    - Ensure that each module has its dedicated functionality and minimizes dependencies on other modules.

