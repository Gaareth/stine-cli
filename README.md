# stine-cli
A CLI Utility for Uni Hamburg STINE.

## Install
#### Compile from source:
- install [rust](https://www.rust-lang.org/learn/get-started) first
```
  git clone https://github.com/Gaareth/stine-cli.git
  cd stine-cli
  cargo build --release
```
then on linux: `cp target/release/stine-cli /usr/local/bin/`

If you want to use this on e.g. a raspberry pi 
1. use [cross](https://github.com/cross-rs/cross), 
2. use the static_ssl feature: `cross build --features static_ssl --target armv7-unknown-linux-gnueabihf --release`
in case you have problems building openssl

#### Use prebuild binaries from the [releases](https://github.com/Gaareth/stine-cli/releases) tab
Note: They might not be completely up-to-date. Hopefully I can fix the github workflow, so that binaries will be build 
automatically also for windows and macos for every new release

## Authenticate
Use your username and password to login. Either use the cli argument (see help) or create a **.stine-env file next to executable** 
with the following contents:
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

### Notify Command
`stine-cli notify` can notify you about certain events.
Available Events:
  - exam-result: Notify about changes of written exams, like status and grade
  - registration-status: Changes in your registered modules, (e.g. accepted, rejected)
  - documents: New STINE documents
  - registration-periods: Start of new registration periods

Currently, the way to use this command should be a scheduled execution of the command, e.g. as a cronjob
Example:
```stine-cli --save_config notify --email_address "<your email>" --email_password "<>" -events <>```
Resulting in emails like:
  ```
    From: <your email>
    To: <your email>
    Subject: Stine Notifier - Update in course results
    [Module name] (N/A -> 4.0)
  ```

In the future, there should be a set of configurable action, which map to the selected events.

The comparison files are currently written relative to the executable to ./notify.
Logs are written to your $homedir. So e.g. /home/[user]/stine-cli.log on linux.


## TODO
- Config file for Notifications
  - Email Auth
  - Events
  - Actions
    - Email
    - stdout
    - File
    - System Notifications
    - ...?
  - custom message
- Grade "b" as in "passed" in table output instead of "-"

## Contributing
Contributions are welcome, as the current state of the code isn't the best D^:
