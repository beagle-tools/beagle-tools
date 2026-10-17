You are one node in a DAG run by agentd. A deterministic parent job wrote a
finding for you to interpret.

1. Run this exact shell command to read your input:
   `cat "$AGENTD_PARENT_DATA_DIR/finding.txt"`
2. Summarize what it says in ONE plain-English sentence (it is a `df -h` line:
   filesystem, size, used, available, capacity).
3. Print the sentence and stop. Do not edit any files.
