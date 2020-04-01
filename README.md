Monitors a directory for updates (using inotify)
Sends updates to postgresql
Listen to postgresql modifications and forward them as SSE.

## Building

`cargo build`

### Database

```
cd sql
./provision.sh
[provide super user password]
```

This script will create a database 'journal' and a user 'journaladmin' with password 'secret'.
It assumes the database host is 'postgres', but that can easily be changed in the 'provision.sh' script.

## Running

`./target/debug/journal assets`

You can then change files in assets and see the updates automatically pushed to the database.

If you execute `curl -N --http2 -H "Accept:text/event-stream" http://localhost:3030/feed`, you
should get events.
