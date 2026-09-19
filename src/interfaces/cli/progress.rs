use indicatif::{ProgressBar, ProgressStyle};

use crate::progress_report;

pub(crate) fn bar(prefix: &'static str, label: &str) -> ProgressBar {
  let bar = ProgressBar::new_spinner();
  bar.set_draw_target(super::progress_draw_target());
  bar.set_style(
    ProgressStyle::with_template(
      "{prefix:.bold.green} {msg:<40}  [{bar:20.green/white}] {percent:>3}%  {pos:>6}/{len:<6}  {per_sec}  eta {eta}",
    )
    .unwrap()
    .progress_chars("=> "),
  );
  bar.set_prefix(prefix);
  bar.set_message(label.to_string());
  bar
}

pub(crate) fn advance(bar: &ProgressBar, report: &progress_report) {
  if let Some(total) = report.total
    && bar.length().is_none()
  {
    bar.set_length(total);
  }
  bar.set_position(report.processed);
}
