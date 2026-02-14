#!/bin/bash
# Flood the terminal with data to test rendering under heavy PTY output.
# Usage: ./test_flood.sh
while true; do
    printf '%0200d\n' $RANDOM$RANDOM$RANDOM$RANDOM
done
