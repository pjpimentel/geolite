use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Margin, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};
use ratatui::{DefaultTerminal, Frame};
use rusqlite::Connection;

use super::filter::filter;
use super::leaf::leaf;
use super::map::{self, zoom};
use super::tree::{folder, item, opened, tree};

const MAX_WIDTH: u16 = 100;
const KEYS: &str = "↑↓ move  ⏎ enter  ⌫ up  / filter  q quit";
const KEYS_FILTERED: &str = "↑↓ move  ⏎ enter  ⌫ up  / filter  esc clear  q quit";
const KEYS_TYPING: &str = "↑↓ move  ⏎ enter  ⌫ erase  esc clear";
const KEYS_LEAF: &str = "+ - zoom  ⌫ up  q quit";
const PROMPT: &str = "  / ";
const PLACEHOLDER: &str = "filter";

struct view<'a> {
  conn: &'a Connection,
  tree: tree,
  folder: folder,
  leaf: Option<Box<leaf>>,
  zoom: zoom,
  filter: filter,
  typing: bool,
  state: TableState,
  page: u16,
}

pub fn run(conn: &Connection, tree: tree, opened: opened) {
  let (folder, leaf) = match opened {
    opened::folder(folder) => (folder, None),
    opened::leaf(folder, leaf) => (folder, Some(leaf)),
  };
  let mut view = view {
    conn,
    tree,
    filter: filter::over(&folder),
    folder,
    leaf: None,
    zoom: zoom::default(),
    typing: false,
    state: TableState::default(),
    page: 1,
  };
  view.select_first_item();
  if let Some(leaf) = leaf {
    view.select_leaf(&leaf);
    view.leaf = Some(leaf);
  }
  ratatui::run(|terminal| view.run(terminal)).expect("failed to run the tui");
}

// `.` and, below the roots, `..` come before the items, as `ls -la` lists them
fn head_rows(folder: &folder) -> usize {
  if folder.path.is_empty() { 1 } else { 2 }
}

impl view<'_> {
  fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
      terminal.draw(|frame| self.draw(frame))?;
      if let Event::Key(key) = event::read()?
        && key.kind == KeyEventKind::Press
        && self.handle(key.code, key.modifiers)
      {
        return Ok(());
      }
    }
  }

  fn draw(&mut self, frame: &mut Frame) {
    let [_, column, _] = Layout::horizontal([
      Constraint::Fill(1),
      Constraint::Max(MAX_WIDTH),
      Constraint::Fill(1),
    ])
    .areas(frame.area());
    match &self.leaf {
      Some(leaf) => self.zoom = draw_leaf(frame, column, leaf, self.zoom),
      None => self.draw_folder(frame, column),
    }
  }

  fn draw_folder(&mut self, frame: &mut Frame, column: Rect) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let block = Block::bordered()
      .title(format!(" {} ", self.folder.breadcrumb()))
      .title_bottom(format!(" {} ", self.keys()));
    let [input, list] =
      Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(block.inner(column));
    self.page = list.height.saturating_sub(1).max(1);
    frame.render_widget(block, column);
    self.draw_input(frame, input);
    let mut rows = vec![self.dot_row()];
    if let Some(above) = self.dotdot_row() {
      rows.push(above);
    }
    rows.extend(
      self
        .filter
        .shown
        .iter()
        .map(|&index| item_row(&self.folder.items[index])),
    );
    let table = Table::new(
      rows,
      [
        Constraint::Length(12),
        Constraint::Length(6),
        Constraint::Fill(1),
      ],
    )
    .header(Row::new(["level", "inside", "name"]).style(dim))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol("> ");
    frame.render_stateful_widget(table, list, &mut self.state);
  }

  fn draw_input(&self, frame: &mut Frame, area: Rect) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let count = if self.filter.text.is_empty() {
      String::new()
    } else {
      format!(
        "{} of {} ",
        self.filter.shown.len(),
        self.folder.items.len()
      )
    };
    let [typed_area, count_area] = Layout::horizontal([
      Constraint::Fill(1),
      Constraint::Length(count.len() as u16),
    ])
    .areas(area);
    let typed = if self.filter.text.is_empty() && !self.typing {
      Line::from(format!("{PROMPT}{PLACEHOLDER}")).style(dim)
    } else {
      Line::from(vec![
        Span::styled(PROMPT, dim),
        Span::raw(self.filter.text.as_str()),
      ])
    };
    if self.typing {
      let after = typed_area.x.saturating_add(typed.width() as u16);
      let last = typed_area.right().saturating_sub(1);
      frame.set_cursor_position(Position::new(after.min(last), typed_area.y));
    }
    frame.render_widget(typed, typed_area);
    frame.render_widget(Line::from(count).style(dim), count_area);
  }

  fn keys(&self) -> &'static str {
    if self.typing {
      KEYS_TYPING
    } else if self.filter.text.is_empty() {
      KEYS
    } else {
      KEYS_FILTERED
    }
  }

  fn dot_row(&self) -> Row<'static> {
    match self.folder.current() {
      Some(item) => cells(item.entry.level().name(), Some(item.inside), "."),
      None => cells("", Some(self.folder.items.len()), "."),
    }
  }

  fn dotdot_row(&self) -> Option<Row<'static>> {
    if self.folder.path.is_empty() {
      return None;
    }
    Some(match self.folder.above() {
      Some(item) => cells(item.entry.level().name(), Some(item.inside), ".."),
      None => cells("", Some(self.tree.root_count), ".."),
    })
  }

  fn handle(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
    if self.leaf.is_some() {
      return self.read(code, modifiers);
    }
    let page = self.page;
    match code {
      KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,
      KeyCode::Up => self.state.select_previous(),
      KeyCode::Down => self.state.select_next(),
      KeyCode::PageUp => self.state.scroll_up_by(page),
      KeyCode::PageDown => self.state.scroll_down_by(page),
      KeyCode::Home => *self.state.selected_mut() = Some(0),
      KeyCode::End => {
        let last = head_rows(&self.folder) + self.filter.shown.len() - 1;
        *self.state.selected_mut() = Some(last);
      }
      KeyCode::Enter => self.enter(),
      KeyCode::Esc => self.clear_filter(),
      _ if self.typing => self.typed(code, modifiers),
      _ => return self.browsed(code),
    }
    false
  }

  fn typed(&mut self, code: KeyCode, modifiers: KeyModifiers) {
    match code {
      KeyCode::Char(typed) if !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
        self.refilter(|filter| filter.push(typed));
      }
      KeyCode::Backspace if self.filter.text.is_empty() => self.typing = false,
      KeyCode::Backspace => self.refilter(filter::pop),
      _ => {}
    }
  }

  fn read(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
    match code {
      KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,
      KeyCode::Char('q') => return true,
      KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => self.leaf = None,
      KeyCode::Char('+') | KeyCode::Char('=') => self.zoom = self.zoom.closer(),
      KeyCode::Char('-') => self.zoom = self.zoom.farther(),
      _ => {}
    }
    false
  }

  fn browsed(&mut self, code: KeyCode) -> bool {
    match code {
      KeyCode::Char('q') => return true,
      KeyCode::Char('k') => self.state.select_previous(),
      KeyCode::Char('j') => self.state.select_next(),
      KeyCode::Char('/') => self.typing = true,
      KeyCode::Right | KeyCode::Char('l') => self.enter(),
      KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => self.up(),
      _ => {}
    }
    false
  }

  // `.` stays, `..` goes up, a folder is entered, the deepest level is opened
  fn enter(&mut self) {
    self.typing = false;
    let Some(row) = self.state.selected() else {
      return;
    };
    let head = head_rows(&self.folder);
    if row == 0 {
      return;
    }
    if row < head {
      self.up();
      return;
    }
    let Some(&index) = self.filter.shown.get(row - head) else {
      return;
    };
    match self.tree.enter(self.conn, &self.folder, index) {
      Some(entered) => {
        self.list(entered);
        self.select_first_item();
      }
      None => {
        self.leaf = super::leaf::open(self.conn, &self.folder, index).map(Box::new);
        self.zoom = zoom::default();
      }
    }
  }

  // going up selects the folder just left, as a file manager does
  fn up(&mut self) {
    let Some(parent) = self.tree.parent(self.conn, &self.folder) else {
      return;
    };
    let left = self.folder.current().cloned();
    let position = left.and_then(|left| parent.items.iter().position(|item| item.same_as(&left)));
    let row = head_rows(&parent) + position.unwrap_or(0);
    self.list(parent);
    *self.state.selected_mut() = Some(row);
  }

  fn list(&mut self, folder: folder) {
    self.filter = filter::over(&folder);
    self.folder = folder;
    self.typing = false;
  }

  fn clear_filter(&mut self) {
    self.typing = false;
    self.refilter(filter::clear);
  }

  fn refilter(&mut self, change: impl FnOnce(&mut filter)) {
    let selected = self.selected_item();
    change(&mut self.filter);
    let kept = selected.and_then(|index| self.filter.shown.binary_search(&index).ok());
    match kept {
      Some(position) => {
        *self.state.selected_mut() = Some(head_rows(&self.folder) + position);
      }
      None => self.select_first_item(),
    }
  }

  fn selected_item(&self) -> Option<usize> {
    let position = self
      .state
      .selected()?
      .checked_sub(head_rows(&self.folder))?;
    self.filter.shown.get(position).copied()
  }

  fn select_first_item(&mut self) {
    let row = if self.filter.shown.is_empty() { 0 } else { head_rows(&self.folder) };
    *self.state.selected_mut() = Some(row);
  }

  fn select_leaf(&mut self, leaf: &leaf) {
    let position = leaf
      .path
      .last()
      .and_then(|opened| self.folder.items.iter().position(|item| item.same_as(opened)));
    if let Some(position) = position {
      *self.state.selected_mut() = Some(head_rows(&self.folder) + position);
    }
  }
}

fn draw_leaf(frame: &mut Frame, column: Rect, leaf: &leaf, zoom: zoom) -> zoom {
  let dim = Style::default().add_modifier(Modifier::DIM);
  let block = Block::bordered()
    .title(format!(" {} ", leaf.breadcrumb()))
    .title_bottom(format!(" {KEYS_LEAF} "));
  let fields = leaf.fields();
  let names = fields.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
  let [data, rule, drawn] = Layout::vertical([
    Constraint::Length(fields.len() as u16),
    Constraint::Length(1),
    Constraint::Fill(1),
  ])
  .areas(block.inner(column));
  frame.render_widget(block, column);
  let rows = fields.into_iter().map(|(name, value)| {
    Row::new([Cell::from(name).style(dim), Cell::from(value)])
  });
  let table = Table::new(rows, [Constraint::Length(names as u16), Constraint::Fill(1)])
    .column_spacing(2);
  frame.render_widget(table, data.inner(Margin::new(1, 0)));
  let zoom = map::settled(&leaf.drawing, drawn, zoom);
  let width = map::ground_width(&leaf.drawing, drawn, zoom);
  let ruled = Block::new()
    .borders(Borders::TOP)
    .border_style(dim)
    .title(Line::from(format!(" ↔ {width} ")).right_aligned());
  frame.render_widget(ruled, rule);
  map::render(&leaf.drawing, zoom, drawn, frame.buffer_mut());
  zoom
}

fn item_row(item: &item) -> Row<'static> {
  let inside = item.is_folder.then_some(item.inside);
  let row = cells(item.entry.level().name(), inside, &item.entry.name());
  if item.entry.is_missing() {
    row.style(Style::default().add_modifier(Modifier::DIM))
  } else {
    row
  }
}

fn cells(level: &str, inside: Option<usize>, name: &str) -> Row<'static> {
  let inside = inside.map(|inside| inside.to_string()).unwrap_or_default();
  Row::new([
    Cell::from(level.to_string()),
    Cell::from(Text::from(inside).right_aligned()),
    Cell::from(name.to_string()),
  ])
}
