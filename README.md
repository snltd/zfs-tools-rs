# ZFS Tools

Rust rewrites of some shell and Ruby scripts I've been using for years. Some of
what they do is now replicated in illumos.

## zfs-real-usage

The way ZFS reports space can be a little confusing: `zfs-real-usage` tells you
very clearly how much real disk space is occupied by your filesystems and
snapshots. It sorts from the least to the most, and takes no options. If you
want to filter, use `rg` or `grep`.

This is useful when you need to clear some space and some deeply buried snapshot
is hogging a stack of room.

## zfs-remove-snaps

Batch-removes ZFS snapshots.

- `-f` (`--files`) specifies that arguments are files. The program will work out
  which filesystems contain them. If you don't supply `-f` or `-s`, then all
  arguments are assumed to be ZFS filesystem names.

- `-a` (`--all-datasets`) tells the program to remove snapshots under all
  filesystems whose name matches any of the arguments. So `-a logs` would remove
  snaps for `rpool/logs` `rpool/application/logs` and `tank/logs`.
- `-s` (`--snaps`) means that all arguments are snapshot names. `-s monday`
  would remove all `@monday` snapshots anywhere in your hierarchy.

- `-r` (`--recurse`) recurses down the filesystem tree. If you have a pool
  called `tank`, `-r tank` would remove every snapshot it. This doesn't use
  ZFS's native `-r`, preferring to work out its own matches. This means you can
  use...

- `-o LIST` (`--omit-fs`) tells the program NOT to delete snapshots belonging to
  filesystems included in a n comma-separated list. Basic wildcards are
  supported, so`\*keep,\*these\*,safe\*`would not remove snapshots from any
  filesystems whose names end with`keep`, or contain `these`, or begin
  with`safe`.

- `-O LIST` (`--omit-snaps`) tells the program NOT to delete any snapshots whose
  names are included in a comma-separated list. You can use `-o` and `-O`
  together, but you can't use them when your arguments are snapshots or dataset
  names. i.e. with `-s` or `-a`.

- `-n` (`--noop`) makes the program print the `zfs` commands it would run,
  without actually running them.

- `-v` (`--verbose`) prints the `zfs` commands as they are run.

## zfs-rogue-snaps

I have a snapshot naming scheme. `zfs-rogue-snaps` finds snapshots which do not
fit that scheme. It probably won't be useful to anyone else, unless, perhaps, if
you use...

## zfs-snap

## zfs-touch-from-snap

## zp

## zr
