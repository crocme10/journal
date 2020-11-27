#!/bin/sh

echo "Waiting for postgres..."

while ! nc -z localhost 5432; do
  sleep 0.1
done

echo "PostgreSQL started"

echo "Hostname: "
hostname
./service run
