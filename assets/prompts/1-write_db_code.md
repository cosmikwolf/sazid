Act as a rust developer.

## Iteration Steps:

To create an iterative development process for the Rust file `./src/app/db/vector_db.rs`, follow these steps, maintaining state between calls as necessary:

1. Use the `file_search` function to check if `vector_db.rs` exists. If it does, use the `read_file` function to read its contents; if not, start with an initial template.
2. Analyze the current contents to determine the missing functionality.
3. Write the additional code needed to progress towards completion.
4. Check the written code for completeness and accuracy.
5. If the code is incomplete, prepare the next iteration of code to add; if it is complete, finalize the file content.
6. During each iteration, consider implementing new functions, improving existing code, and adding or refining tests.
7. When implementing functionality, utilize the tokio_postgres crate to interface with the database. Example code is below
8. When implementing SQL code, since we are using a postgres extension called pgvecto.rs, we will need to use special SQL queries. Instructions on using this database are below.

As an iterative prompt engineer, execute these steps. Below is the prompt for GPT to execute step 1:

```plaintext
Use the `file_search` function to check if `vector_db.rs` exists. If it does, read its contents to determine what portions of the intended functionality have been implemented. If the file does not exist, begin drafting the initial template of the required Rust module as described previously. If overwriting is necessary, set the `overwrite` parameter to `true` in the `create_file` function call.
```




Proceed with the iterative development process, adding one function at a time, refining each based on the project's described requirements, and ensure all code changes are saved with `create_file`, using `overwrite` as true for updates after the initial file creation. This iterative approach will build up the code until all functionality is incorporated and tested, and the code is deemed complete.


Example tokio_postgres code
```rust
use tokio_postgres::{NoTls, Error};

#[tokio::main] // By default, tokio_postgres uses the tokio crate as its runtime.
async fn main() -> Result<(), Error> {
    // Connect to the database.
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=postgres", NoTls).await?;

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    // Now we can execute a simple statement that just returns its parameter.
    let rows = client
        .query("SELECT $1::TEXT", &[&"hello world"])
        .await?;

    // And then check that we got back the same string we sent over.
    let value: &str = rows[0].get(0);
    assert_eq!(value, "hello world");

    Ok(())
}
```


```markdown
# instructions on interacting with the pgvecto postgres vector database

Run the following SQL to ensure the extension is enabled.

```sql
DROP EXTENSION IF EXISTS vectors;
CREATE EXTENSION vectors;
```

pgvecto.rs introduces a new data type `vector(n)` denoting an n-dimensional vector. The `n` within the brackets signifies the dimensions of the vector.

You could create a table with the following SQL.

```sql
-- create table with a vector column

CREATE TABLE items (
  id bigserial PRIMARY KEY,
  embedding vector(3) NOT NULL -- 3 dimensions
);
```

You can then populate the table with vector data as follows.

```sql
-- insert values

INSERT INTO items (embedding)
VALUES ('[1,2,3]'), ('[4,5,6]');

-- or insert values using a casting from array to vector

INSERT INTO items (embedding)
VALUES (ARRAY[1, 2, 3]::real[]), (ARRAY[4, 5, 6]::real[]);
```

We support three operators to calculate the distance between two vectors.

- `<->`: squared Euclidean distance, defined as $\Sigma (x_i - y_i) ^ 2$.
- `<#>`: negative dot product, defined as $- \Sigma x_iy_i$.
- `<=>`: negative cosine similarity, defined as $- \frac{\Sigma x_iy_i}{\sqrt{\Sigma x_i^2 \Sigma y_i^2}}$.

```sql
-- call the distance function through operators

-- squared Euclidean distance
SELECT '[1, 2, 3]'::vector <-> '[3, 2, 1]'::vector;
-- negative dot product
SELECT '[1, 2, 3]' <#> '[3, 2, 1]';
-- negative cosine similarity
SELECT '[1, 2, 3]' <=> '[3, 2, 1]';
```

You can search for a vector simply like this.

```sql
-- query the similar embeddings
SELECT * FROM items ORDER BY embedding <-> '[3,2,1]' LIMIT 5;
```

## Things You Need to Know

`vector(n)` is a valid data type only if $1 \leq n \leq 65535$. Due to limits of PostgreSQL, it's possible to create a value of type `vector(3)` of $5$ dimensions and `vector` is also a valid data. However, you cannot still put $0$ scalar or more than $65535$ scalars to a vector. If you use `vector` for a column or there is some values mismatched with dimension denoted by the column, you won't able to create an index on it.

```
