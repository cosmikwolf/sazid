Act as a rust developer.

## Iteration Steps:

To create an iterative development process for the Rust file `./src/app/vector_db.rs`, follow these steps, maintaining state between calls as necessary:

1. Use the `file_search` function to check if `vector_db.rs` exists. If it does, use the `read_file` function to read its contents; if not, start with an initial template.
2. Analyze the current contents to determine the missing functionality.
3. Write the additional code needed to progress towards completion.
4. Check the written code for completeness and accuracy.
5. If the code is incomplete, prepare the next iteration of code to add; if it is complete, finalize the file content.
6. During each iteration, consider implementing new functions, improving existing code, and adding or refining tests.
7. When implementing functionality, utilize the tokio-postgres crate to interface with the database. Example code is below
8. When implementing SQL code, since we are using a postgres extension called pgvecto.rs, we will need to use special SQL queries. Instructions on using this database are below.
9. The vector_db.rs file should provide functionality to leverage all of the functionality that pgvecto.rs provides, as exampled below.
10. Ensure that when you overwrite a file, you include all code in the new file. Omitting any code will result in an incomplete source file

As an iterative prompt engineer, execute these steps. Below is the prompt for GPT to execute step 1:

```plaintext
Use the `file_search` function to check if `vector_db.rs` exists. If it does, read its contents to determine what portions of the intended functionality have been implemented. If the file does not exist, begin drafting the initial template of the required Rust module as described previously. If overwriting is necessary, set the `overwrite` parameter to `true` in the `create_file` function call.
```




Proceed with the iterative development process, adding one function at a time, refining each based on the project's described requirements, and ensure all code changes are saved with `create_file`, using `overwrite` as true for updates after the initial file creation. This iterative approach will build up the code until all functionality is incorporated and tested, and the code is deemed complete.


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

# Indexing

Indexing is the core ability of pgvecto.rs.

Assuming there is a table `items` and there is a column named `embedding` of type `vector(n)`, you can create a vector index for squared Euclidean distance with the following SQL.

```sql
CREATE INDEX ON items USING vectors (embedding l2_ops);
```

For negative dot product, replace `l2_ops` with `dot_ops`.
For negative cosine similarity, replace `l2_ops` with `cosine_ops`.

Now you can perform a KNN search with the following SQL again, but this time the vector index is used for searching.

```sql
SELECT * FROM items ORDER BY embedding <-> '[3,2,1]' LIMIT 5;
```

## Things You Need to Know

pgvecto.rs constructs the index asynchronously. When you insert new rows into the table, they will first be placed in an append-only file. The background thread will periodically merge the newly inserted row to the existing index. When a user performs any search prior to the merge process, it scans the append-only file to ensure accuracy and consistency.

## Options

We utilize TOML syntax to express the index's configuration. Here's what each key in the configuration signifies:

| Key        | Type  | Description                            |
| ---------- | ----- | -------------------------------------- |
| segment    | table | Options for segments.                  |
| optimizing | table | Options for background optimizing.     |
| indexing   | table | The algorithm to be used for indexing. |

Options for table `segment`.

| Key                      | Type    | Description                                                         |
| ------------------------ | ------- | ------------------------------------------------------------------- |
| max_growing_segment_size | integer | Maximum size of unindexed vectors. Default value is `20_000`.       |
| min_sealed_segment_size  | integer | Minimum size of vectors for indexing. Default value is `1_000`.     |
| max_sealed_segment_size  | integer | Maximum size of vectors for indexing. Default value is `1_000_000`. |

Options for table `optimizing`.

| Key                | Type    | Description                                                                 |
| ------------------ | ------- | --------------------------------------------------------------------------- |
| optimizing_threads | integer | Maximum threads for indexing. Default value is the sqrt of number of cores. |

Options for table `indexing`.

| Key  | Type  | Description                                                             |
| ---- | ----- | ----------------------------------------------------------------------- |
| flat | table | If this table is set, brute force algorithm will be used for the index. |
| ivf  | table | If this table is set, IVF will be used for the index.                   |
| hnsw | table | If this table is set, HNSW will be used for the index.                  |

You can choose only one algorithm in above indexing algorithms. Default value is `hnsw`.

Options for table `flat`.

| Key          | Type  | Description                                |
| ------------ | ----- | ------------------------------------------ |
| quantization | table | The algorithm to be used for quantization. |

Options for table `ivf`.

| Key              | Type    | Description                                                     |
| ---------------- | ------- | --------------------------------------------------------------- |
| nlist            | integer | Number of cluster units. Default value is `1000`.               |
| nprobe           | integer | Number of units to query. Default value is `10`.                |
| least_iterations | integer | Least iterations for K-Means clustering. Default value is `16`. |
| iterations       | integer | Max iterations for K-Means clustering. Default value is `500`.  |
| quantization     | table   | The quantization algorithm to be used.                          |

Options for table `hnsw`.

| Key             | Type    | Description                                        |
| --------------- | ------- | -------------------------------------------------- |
| m               | integer | Maximum degree of the node. Default value is `12`. |
| ef_construction | integer | Search scope in building. Default value is `300`.  |
| quantization    | table   | The quantization algorithm to be used.             |

Options for table `quantization`.

| Key     | Type  | Description                                         |
| ------- | ----- | --------------------------------------------------- |
| trivial | table | If this table is set, no quantization is used.      |
| scalar  | table | If this table is set, scalar quantization is used.  |
| product | table | If this table is set, product quantization is used. |

You can choose only one algorithm in above indexing algorithms. Default value is `trivial`.

Options for table `product`.

| Key    | Type    | Description                                                                                                              |
| ------ | ------- | ------------------------------------------------------------------------------------------------------------------------ |
| sample | integer | Samples to be used for quantization. Default value is `65535`.                                                           |
| ratio  | string  | Compression ratio for quantization. Only `"x4"`, `"x8"`, `"x16"`, `"x32"`, `"x64"` are allowed. Default value is `"x4"`. |

## Progress View

We also provide a view `pg_vector_index_info` to monitor the progress of indexing.
Note that whether idx_sealed_len is equal to idx_tuples doesn't relate to the completion of indexing.
It may do further optimization after indexing. It may also stop indexing because there are too few tuples left.

| Column          | Type   | Description                                   |
| --------------- | ------ | --------------------------------------------- |
| tablerelid      | oid    | The oid of the table.                         |
| indexrelid      | oid    | The oid of the index.                         |
| tablename       | name   | The name of the table.                        |
| indexname       | name   | The name of the index.                        |
| indexing        | bool   | Whether the background thread is indexing.    |
| idx_tuples      | int4   | The number of tuples.                         |
| idx_sealed_len  | int4   | The number of tuples in sealed segments.      |
| idx_growing_len | int4   | The number of tuples in growing segments.     |
| idx_write       | int4   | The number of tuples in write buffer.         |
| idx_sealed      | int4[] | The number of tuples in each sealed segment.  |
| idx_growing     | int4[] | The number of tuples in each growing segment. |
| idx_config      | text   | The configuration of the index.               |

## Examples

There are some examples.

```sql
-- HNSW algorithm, default settings.

CREATE INDEX ON items USING vectors (embedding l2_ops);

--- Or using bruteforce with PQ.

CREATE INDEX ON items USING vectors (embedding l2_ops)
WITH (options = $$
[indexing.flat]
quantization.product.ratio = "x16"
$$);

--- Or using IVFPQ algorithm.

CREATE INDEX ON items USING vectors (embedding l2_ops)
WITH (options = $$
[indexing.ivf]
quantization.product.ratio = "x16"
$$);

-- Use more threads for background building the index.

CREATE INDEX ON items USING vectors (embedding l2_ops)
WITH (options = $$
optimizing.optimizing_threads = 16
$$);

-- Prefer smaller HNSW graph.

CREATE INDEX ON items USING vectors (embedding l2_ops)
WITH (options = $$
segment.max_growing_segment_size = 200000
$$);
```


# Searching

The SQL will fetch the $5$ nearest embedding in table `items`.

```sql
SELECT * FROM items ORDER BY embedding <-> '[3,2,1]' LIMIT 5;
```

## Things You Need to Know

If `vectors.k` is set to `64`, but your SQL returned less than `64` rows, for example, only `32` rows. There is some possible reasons:

* Less than `64` rows should be returned. It's expected.
* The vector index returned `64` rows, but `32` of which are deleted before but the index do not know since PostgreSQL vacuum is lazy.
* The vector index returned `64` rows, but `32` of which are invisble to the transaction so PostgreSQL decided to hide these rows for you.
* The vector index returned `64` rows, but `32` of which are satifying the condition `id % 2 = 0` in `WHERE` clause.

There are three ways to solve the problem:

* Set `vectors.k` larger. If you estimate that 20% of rows will satisfy the condition in `WHERE`, just set `vectors.k` to be 5 times than before.
* Set `vectors.enable_vector_index` to `off`. If you estimate that 0.0001% of rows will satisfy the condition in `WHERE`, just do not use vector index. No alogrithms will be faster than brute force by PostgreSQL.
* Set `vectors.enable_prefilter` to `on`. If you cannot estimate how many rows will satisfy the condition in `WHERE`, leave the job for the index. The index will check if the returned row can be accepted by PostgreSQL. However, it will make queries slower so the default value for this option is `off`.

## Options

Search options are specified by PostgreSQL GUC. You can use `SET` command to apply these options in session or `SET LOCAL` command to apply these options in transaction.

| Option                      | Type    | Description                                                                                                                                                   |
| --------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| vectors.k                   | integer | Expected number of candidates returned by index. The parameter will influence the recall if you use HNSW or quantization for indexing. Default value is `64`. |
| vectors.enable_prefilter    | boolean | Enable prefiltering or not. Default value is `off`.                                                                                                           |
| vectors.enable_vector_index | boolean | Enable vector indexes or not. This option is for debugging. Default value is `on`.                                                                            |
```
