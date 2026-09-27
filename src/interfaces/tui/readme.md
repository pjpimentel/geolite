# tui
> `geolite tui [path]`: the admin levels walked like folders, one at a time, from the country down to the streets

the tui opens at the roots, or at `path`: the names of the folders from a root, separated by `/`
and matched without regard to case, as in `geolite tui "Brasil/São Paulo/Santos"`. it lists one
folder at a time, the way `ls -la` does: `.` is the folder itself, `..` the one above it, and then
what hangs from the area in the hierarchy, each entry with its level, the places inside it and its
name. entering a folder replaces the listing with its own, and going up brings the listing above
back, the folder just left selected. a street never contains anything, so it is never entered.

## the ladder

the levels the database holds make a ladder, `country / state / city / neighborhood / street`
under the brazil preset. under a folder the ladder expects the next level: the places deeper than
that lack a level, so a state lists its cities, and its neighbourhoods and streets outside any
city go first into a folder of their own, `no city`; inside it the rule applies again, and the
streets outside any neighbourhood go into `no neighborhood`. a nested area of the same level as its
parent is listed as any other folder. a folder counts the places directly inside it; the `no
<level>` folder counts what it holds.

## the filter

the first line of the listing is an input. `/` focuses it, and what is typed keeps only the entries
whose name contains it, both folded the way the search index folds a text, without regard to case
or diacritics: `sao` finds `São Vicente`. `.` and `..` always stay, and the input counts the
entries shown against the ones the folder holds. the selected entry stays selected while it is
still shown; otherwise the first one shown is.

while the input is focused the letters are typed, so only the arrows and the jumps move. `Enter`
leaves the input and enters the selected folder, `Backspace` erases, and leaves an input already
empty, and `Esc` clears the filter. the filter belongs to the folder listed: entering a folder or
going up starts with none.

## the keys

| key | does |
|---|---|
| `↑` `↓`, `k` `j` | move |
| `PageUp` `PageDown`, `Home` `End` | jump |
| `Enter` `→` `l` | enter the selected folder, or go up on `..` |
| `Backspace` `←` `h` | go up |
| `/` | type in the filter |
| `Esc` | clear the filter |
| `q`, `Ctrl-C` | quit |

the listing sits in a column centred on the screen, at most 100 columns wide and as tall as the
terminal; the title names the folder listed.

when stdout is not a terminal the tui prints the folder at `path` and exits, the same columns
without `.` and `..`: a header, a line of dashes and one entry per line, the `no <level>` folder
first when there is one, or `no places under <folder>`. that is how the e2e battery walks the
tree, and how a shell script reads it.

without data — no database, or a database whose hierarchy was never built — the tui prints
`first you need to build the data using geolite build` and exits with 1.

`tree` is the model (the ladder and the counts; `roots`, `enter`, `parent` and `resolve` answer
a `folder`, its path and its items), `filter` keeps the items of a folder whose name contains the
text typed, `listing` prints a folder, `view` draws it with ratatui; the cli's `tui.rs` checks the
data, resolves the path and picks one of the two.
