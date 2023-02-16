# stine-cli
A CLI Utility for Uni Hamburg STINE.

## Authenticate
Use your username and password to login. Either use the cli argument (see help) or create a .env file next to executable.
``` 
username = <your username>
password = <your password>
```
The programm will then try to save a session cookie in this file to simplify further logins. (when using --save_config)
Using the cli args will be prioritized.

## Commands
Currently, the following subcommands are available:
```
Commands:
  semester-results     Print exam results of semesters
  registration-status  Print registration status of all applied (sub)-modules
  notify               Send email about various events
  check                Check your credentials and connection to Stine
  help                 Print this message or the help of the given subcommand(s)
```
For more info use `stine-cli help <subcommand>`

## TODO
- Config file for Notifications
  - Email Auth
  - Events
  - Actions
  - custom message


## Contributing
Contributions are welcome, as the current state of the code isn't the best D^: