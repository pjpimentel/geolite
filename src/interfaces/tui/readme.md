# tui
> `geolite tui [path]`: the admin levels walked like folders, one at a time, from the country down to the streets, and the address and the map of the one opened at the end

the tui opens at the roots, or at `path`: the names of the folders from a root, separated by `/`
and matched without regard to case, as in `geolite tui "Brasil/São Paulo/Santos"`. it lists one
folder at a time, the way `ls -la` does: `.` is the folder itself, `..` the one above it, and then
what hangs from the area in the hierarchy, each entry with its level, the places inside it and its
name. entering a folder replaces the listing with its own, and going up brings the listing above
back, the folder just left selected. the deepest level holds nothing to list, so it is never
entered: it is opened.

## the ladder

the levels the database holds make a ladder, `country / state / city / neighborhood / street`
under the brazil preset. under a folder the ladder expects the next level: the places deeper than
that lack a level, so a state lists its cities, and its neighbourhoods and streets outside any
city go first into a folder of their own, `no city`; inside it the rule applies again, and the
streets outside any neighbourhood go into `no neighborhood`. a nested area of the same level as its
parent is listed as any other folder. a folder counts the places directly inside it; the `no
<level>` folder counts what it holds.

## the leaf

the deepest level of the ladder is the leaf, the street under the brazil preset and the city in a
base extracted without streets. it is listed counting nothing, and opening it replaces the listing
with the address of the path walked down to it, above a map of it. the address is the one the api
answers for that path, read through `address::at_path`: the name, the label, the post code and
the country when the path has them, the point, the osm way or relation (the ways, for a street
folded from several), the id of the path and the levels above. a `no <level>` folder is not an
area, so it is not part of the path: a street opened from `no neighborhood` answers under its city.
an area of the deepest level nested inside another of that level is not reached, since the one
around it is opened instead of listed.

the map draws the hierarchy of the leaf, read from the geometries of the rows: the outline of
every area of the path, from the country down to the one the leaf was opened from, in braille dots,
and over them the leaf itself in half blocks, which is what tells it apart without a colour. it
opens framed on the nearest area, with a margin, so it shows where the street runs inside its
neighbourhood, and the areas above are seen only where they cross that frame; a leaf opened from
the roots has no area around and is framed on itself. the frame takes the same ground per cell
width on both axes — a degree of longitude shortened by the cosine of the latitude of the nearest
area, a cell counted twice as tall as it is wide — so a square on the ground is a square on the
screen.

`+` and `-` zoom the map, each step doubling or halving the ground a cell holds. zooming out ends
at the whole hierarchy: the last step frames the largest area of the path, centred, and a `-`
after it changes nothing. zooming in goes sixteen steps past the opening frame, far enough to read
the shape of a street opened from a state. the zoom holds the centre of the leaf where it is on the
screen and never frames past the hierarchy, so the leaf stays on the map while the areas around it
come into it one after the other. the ground the map is wide is written at the end of the rule
above it, as `↔ 3.6 km`. a leaf always opens at the nearest area, which is also the map printed
without a terminal.

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
| `Enter` `→` `l` | enter the selected folder, open the selected leaf, or go up on `..` |
| `Backspace` `←` `h` | go up, or close the leaf |
| `/` | type in the filter |
| `+` `=`, `-` | zoom the map of the leaf in, or out |
| `Esc` | clear the filter, or close the leaf |
| `q`, `Ctrl-C` | quit |

closing a leaf brings back the listing as it was, the filter and the selection kept.

the listing sits in a column centred on the screen, at most 100 columns wide and as tall as the
terminal; the title names the folder listed, or the path of the leaf opened. the leaf takes the
same column, the address above a rule and the map in the rows left below it.

when stdout is not a terminal the tui prints the folder at `path` and exits, the same columns
without `.` and `..`: a header, a line of dashes and one entry per line, the `no <level>` folder
first when there is one, or `no places under <folder>`. a `path` that ends at a leaf prints the
leaf: one field per line, the name and the value two spaces apart, a blank line, and the map drawn
as text, 98 columns by 24 rows with the trailing blanks cut. a `path` that goes on below a leaf is
refused. that is how the e2e battery walks the tree, and how a shell script reads it.

without data — no database, or a database whose hierarchy was never built — the tui prints
`first you need to build the data using geolite build` and exits with 1.

`tree` is the model (the ladder and the counts; `roots`, `enter` and `parent` answer a `folder`,
its path and its items, and `resolve` answers what a path opens, a folder or a leaf), `leaf` is
the leaf opened (its path, its address and the drawing of its map), `map` holds the drawing, the
shapes of the leaf and of the areas above it, frames it under a `zoom` and draws it, on the screen
or as text, `filter` keeps the items of a folder
whose name contains the text typed, `listing` prints a folder or a leaf, `view` draws them with
ratatui; the cli's `tui.rs` checks the data, resolves the path and picks one of the two.
