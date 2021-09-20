# qualinvest_server

This library is part of a set of tools for quantitative investments.
For mor information, see [qualinvest on github](https://github.com/xemwebe/qualinvest)

Once you have set-up a fresh database (or cleaned an existing database), you need to manually add
a new admin to start working with the empty database. Login in into the database with `psql`,
connect to your database, and add a new user with the following SQL query (make sure to choose
a proper password!)

```SQL
INSERT INTO users (name, display, salt_hash, is_admin)
VALUES ('admin', 'Admin', crypt('admin123',gen_salt('bf',8)), TRUE);
```

License: MIT OR Apache-2.0
