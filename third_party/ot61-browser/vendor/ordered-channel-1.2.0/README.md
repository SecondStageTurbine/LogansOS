# Ordered channel

An MPSC blocking channel that returns messages in sorted sequential consecutive order (explicitly assigned to each message), regardless of the order they were sent in.

It's convenient for collecting order-sensitive results of parallel operations (fork-join iteration).

It's backed by a `BinaryHeap` that's used only when necessary to reorder the messages.
