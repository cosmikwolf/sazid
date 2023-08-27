I would like to simplify this application significantly.

I would like you to understand what I am saying and then summarize the application in full detail.

I would like to make a Rust CLI app 

The CLI app, when launched will start an interactive chat that connects to gpt-3.5 turbo 

The app will use the aynsc_openai crate to connect to gpt

API keys will be stored in environment variables.

full logs of the chat will be stored in a folder called logs

the log file will be a plain text log file that outputs the request sent to the API, as well as the response received. 

The log will be named by date and time of the API call. 

Each time a request / response pair is executed, a new log will be created.


I have some changes to make in the application. 

I would like to change the concept of projects to sessions instead. 

I would like to make the CLI have these commands:
    no arguments
        start interactive mode
        when starting interactive mode, it should load the last used session, I will describe how later.
    --session <session ID>
        this will load a previously executed session and continue from where it left off, starting a new session 
    --list-sessions
        this will go through each session in the sessions folder, and go through the logs
            it will print out each session by ID, the date started, and total tokens used 

All sessions will be stored in a folder called sessions

Each session will be stored in a new folder that is named with the date the session started, so that when alphabetically sorted the, the session folders will be in order.

Each session will have a json file that stores information about that session
    session start time
    session ID
    number of interactions in the session
    if the session was started 

Each session folder will have the directories workspace_data and logs
    workspace_data will copies of the original file that has been uploaded to the API
    logs will contain full logs of the interactions with the openai api. 
    logs will be named with date and time log was created, so when sortable alphabetically so that they are listed in order created
    A new log is created for each




 