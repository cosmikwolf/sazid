Problems to fix
--------
Submitting input seems to lock up the UI for longer periods of time with each request.
Need to check for what blocking tasks occur after enter is hit in input mode, but before the mode is actually changed.

need to add session save / recall

need to add line numbers to grep output

need to add multi line grep with regex


## need to add a timeout check if it is expecting another message

Acceptance tests
----------------

- invoking CLI in various scenarios
    - With a multiple line pipe input:
        echo "test\nline" | cargo run -- -n
        ensure that the lines are both ingested in a single message
    - With a single line pipe input:
        echo "test" | cargo run -- -n
    - With a file input:
        cargo run -- -n < test.txt
    - With a file input and a pipe input:
        echo "test" | cargo run -- -n < test.txt
    - with inputs of all types that are too large to fit in a single chunk
        - ensure that the chunks are split up correctly
        - ensure that an error is produced when the whole message is too large for the model
    - invoking CLI with various options
        - -n (new session)
        - -c (continue session)
        - -m (model)
        - -h (help)
        - combinations of all of these simultaneously


