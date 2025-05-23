# ZFS Tools

[![Test](https://github.com/snltd/zfs-tools-rs/actions/workflows/test.yml/badge.svg)](https://github.com/snltd/zfs-tools-rs/actions/workflows/test.yml)

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

This program takes ZFS snapshots with an automated naming scheme.

- `-t` (`--type`) specifies the format of the snapshot name, Choose from `day`,
  which uses the day of the week, lowercased; `month`; `date`, which is
  formatted `YYYY-mm-dd`; `time`, formatted `HH:MM`; and `now`, which formats
  the current time as `YYYY-mm-dd_HH:MM:SS`.

- `-f (`--files`) has the program work out the ZFS filesystem name from a file
  path.

- `-r` (`--recurse`) recurses down ZFS hierarchies.

- `-o` (`--omit`) lets you specify filesystems which will NOT be snapshotted.
  This is applied after any recursion is calculated. You can use asterisks as
  wildcards in the same way as `zfs-remove-snaps`.

- `-n` (`--noop`) makes the program print the `zfs` commands it would run,
  without actually running them.

- `-v` (`--verbose`) prints the `zfs` commands as they are run.

Existing snapshots with the same names are removed.

## zfs-touch-from-snap

Compares a live filesystem with one of its snapshots, and modifies the mtimes of
the live files, using the snapshot contents as a reference.

- `-s SNAPSHOT` (`--snapname`) tells the program which snapshot to use. If you
  do not supply one, it will assume you have snapshots `monday` through
  `sunday`, and use yesterday's.

- `-n` (`--noop`) prints the actions it would take, without actually taking
  them.

- `-v` (`--verbose`) prints the actions it takes, as it takes them.

## zp

Promotes files from a ZFS snapshot. Specify the file inside the snapshot
directory, for instance `zp /tank/.zfs/snapshot/monday/my/example/file`, and it
will copy `my/example/file` relative to the mounted filesystem root. It is a
less-useful companion to `zr`.

`zp` is automatically recursive: promoting a directory promotes it all the way
down.

- `-N` (`--noclobber`) by default, `zp` will overwrite any existing files. Use
  this option to preserve them.

* `-n` (`--noop`) prints actions without actually taking them.

* `-v` (`--verbose`) prints actions as they are taken.

## zr

Recovers files from ZFS snapshots. Give it a filename, and it will find all
copies of said file in snapshots, and display them in a list with their size and
time of last modification. Pick the one you want, and it will be copied into its
correct place in the live filesystem. Works on files and directories.

- `-a` (`--auto`) will make `zr` recover the most recently modified file rather
  than showing you a list and prompting for input,

- `-N` (`--noclobber`) by default, `zr` will overwrite any existing files. Use
  this option to preserve them. This can be useful if you want to recover lost
  files in a directory without getting back old versions of things which have
  changed.

* `-n` (`--noop`) prints actions without actually taking them.

* `-v` (`--verbose`) prints actions as they are taken.

[Here is an article](https://tech.id264.net/post/2019-04-04-zr) about the original
Ruby versions of `zr` and `zp`.
