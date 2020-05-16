# qualinvest

Qualinvest is a tool box for quantitative analysis and management of financial asset portfolios.
Currently, it consists of three components:

* qualinvest_core: A library implementing the core functionality used by the binary tools. The library supports parsing automatic transaction documents (currently only from comdirect Bank), Bank account and user management, position and P&L calculations. Data is stored persistently in PostgreSQL database. The library relies heavily on [finql](https://crates.io/crates/finql), another rust library providing methods for financial analysis.

* qualinvest_cli: A command line interface to `qualinvest_core`. It supports commands for database renewal, insertion of objects into the database, uploading and parsing documents, and updating of market data quotes. It is mainly useful as an administration tool and for automating processes, e.g. updating market data once a day or every few hours.

* qualinvest_server: A http based GUI interface with multiple user support. Each user has only access to the specified set of accounts and the transactions in that accounts. Currently, view and management of transactions is supported as well as viewing position and P&L. 

Please note that the toolbox is still in an early development stage and will be extended by more useful functionality in the near future. Though, if you miss a feature, please send me a note to help me prioritize the future developments.
