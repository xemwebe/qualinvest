CREATE TABLE IF NOT EXISTS assets (
                id SERIAL PRIMARY KEY,
                asset_class VARCHAR(20) NOT NULL
            );

CREATE TABLE IF NOT EXISTS currencies (
                    id INTEGER PRIMARY KEY,
                    iso_code CHAR(3) NOT NULL UNIQUE,
                    rounding_digits INT NOT NULL,
                    FOREIGN KEY(id) REFERENCES assets(id)
                );

create table if not exists stocks (
                  id INTEGER primary key,
                  name text not null unique,
                  wkn CHAR(6) unique,
                  isin CHAR(12) unique,
                  note text,
                  foreign key(id) references assets(id)
                );

CREATE TABLE IF NOT EXISTS transactions (
                id SERIAL PRIMARY KEY,
                trans_type TEXT NOT NULL,
                asset_id INTEGER,
                cash_amount FLOAT8 NOT NULL,
                cash_currency_id INT NOT NULL,
                cash_date DATE NOT NULL,
                related_trans INTEGER,
                position FLOAT8,
                note TEXT,
                FOREIGN KEY(asset_id) REFERENCES assets(id),
                FOREIGN KEY(cash_currency_id) REFERENCES currencies(id),
                FOREIGN KEY(related_trans) REFERENCES transactions(id)
            );

CREATE TABLE IF NOT EXISTS ticker (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                asset_id INTEGER NOT NULL,
                source TEXT NOT NULL,
                priority INTEGER NOT NULL,
                currency_id INT NOT NULL,
                factor FLOAT8 NOT NULL DEFAULT 1.0,
                tz TEXT,
                cal TEXT,
                FOREIGN KEY(asset_id) REFERENCES assets(id),
                FOREIGN KEY(currency_id) REFERENCES currencies(id)
            );

CREATE TABLE IF NOT EXISTS quotes (
                id SERIAL PRIMARY KEY,
                ticker_id INTEGER NOT NULL,
                price FLOAT8 NOT NULL,
                time TIMESTAMP WITH TIME ZONE NOT NULL,
                volume FLOAT8,
                FOREIGN KEY(ticker_id) REFERENCES ticker(id) 
            );

CREATE TABLE IF NOT EXISTS objects (
            id TEXT PRIMARY KEY,
            object JSON NOT NULL);

CREATE TABLE IF NOT EXISTS users (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                display TEXT NOT NULL,
                salt_hash TEXT NOT NULL,
                is_admin BOOLEAN NOT NULL DEFAULT False,
                UNIQUE (name));

CREATE TABLE IF NOT EXISTS accounts (
                id SERIAL PRIMARY KEY,
                broker TEXT NOT NULL,
                account_name TEXT NOT NULL,
                UNIQUE (broker, account_name));

CREATE TABLE IF NOT EXISTS account_transactions (
                id SERIAL PRIMARY KEY,
                account_id INTEGER NOT NULL,
                transaction_id INTEGER NOT NULL,
                FOREIGN KEY(account_id) REFERENCES accounts(id),
                FOREIGN KEY(transaction_id) REFERENCES transactions(id));

CREATE TABLE IF NOT EXISTS account_rights (
            id SERIAL PRIMARY KEY,
            user_id INTEGER NOT NULL,
            account_id INTEGER NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id),
            FOREIGN KEY(account_id) REFERENCES accounts(id));

CREATE TABLE IF NOT EXISTS user_settings (
                id SERIAL PRIMARY KEY,
                user_id INTEGER UNIQUE,
                settings JSON,
                FOREIGN KEY(user_id) REFERENCES users(id));

CREATE TABLE IF NOT EXISTS documents (
                id SERIAL PRIMARY KEY,
                transaction_id INTEGER NOT NULL,
                hash TEXT NOT NULL,
                path TEXT NOT NULL,
                FOREIGN KEY(transaction_id) REFERENCES transactions(id));

CREATE TABLE pdf_files (
                id int4 NOT NULL,
                pdf bytea NOT NULL,
                FOREIGN KEY (id) REFERENCES documents(id));

insert
	into
	users (id,
	name,
	display,
	salt_hash,
	is_admin)
values (0,
'admin',
'Administrator',
crypt('admin',
gen_salt('bf')),
true);
