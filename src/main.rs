use std::{
    cmp::{max, min},
    collections::HashSet,
    fs::{self},
    time::Instant,
};

use directories_next::ProjectDirs;
use htils::{CharAt, ternary};
use once_cell::sync::Lazy;
use random_word::Lang;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode},
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::*,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListState, Paragraph, Widget},
};
use serde::{Deserialize, Serialize};

struct App<'a> {
    exit: bool,
    app_state: AppState,
    current_word: &'a str,
    input: String,
    start: Option<Instant>,
    finished_time: Option<f32>,
    wrong_input_chars: HashSet<usize>,
    words_limit: usize,
    lang: Lang,
    words: Vec<Word<'a>>,
    wrong_words: HashSet<usize>,
    settings_changed: bool,
    selected_setting: SelectedSetting,
    temp_lang: Lang,
    temp_limit: String,
}

#[derive(Default, PartialEq)]
enum SelectedSetting {
    #[default]
    Lang,
    Limit,
}

#[derive(Default)]
enum AppState {
    #[default]
    Input,
    Pause(Instant),
    Results(ListState),
    Settings,
}

impl<'a> Default for App<'a> {
    fn default() -> Self {
        Self {
            exit: false,
            app_state: AppState::Input,
            current_word: "",
            input: String::new(),
            wrong_input_chars: HashSet::new(),
            words_limit: 50,
            lang: Lang::En,
            words: Vec::new(),
            wrong_words: HashSet::new(),
            start: None,
            finished_time: None,
            settings_changed: false,
            selected_setting: SelectedSetting::default(),
            temp_lang: Lang::En,
            temp_limit: "50".to_string(),
        }
    }
}

impl<'a> App<'a> {
    fn from(config: &Config) -> Self {
        let mut app = Self::default();
        app.lang = get_lang(&config.lang).unwrap_or(Lang::En);
        app.current_word = random_word::get(app.lang);
        app.words_limit = config.limit;
        app.temp_lang = app.lang;
        app.temp_limit = app.words_limit.to_string();
        app
    }

    fn restart(&mut self) {
        self.app_state = AppState::Input;
        self.input.clear();
        self.wrong_input_chars.clear();
        self.words.clear();
        self.wrong_words.clear();
        self.start = None;
        self.finished_time = None;
        self.new_word();
    }

    fn pause(&mut self) {
        self.app_state = AppState::Pause(Instant::now())
    }

    fn resume(&mut self) {
        if let AppState::Pause(paused_at) = self.app_state {
            if let Some(started) = self.start {
                let pause_duration = Instant::now().duration_since(paused_at);
                self.start = Some(started.checked_add(pause_duration).unwrap_or(started));
            }
        }
        self.app_state = AppState::Input;
    }

    fn start(&mut self) {
        self.start = Some(Instant::now())
    }

    fn finish(&mut self) {
        self.finished_time = Some(
            (Instant::now()
                .duration_since(self.start.unwrap())
                .as_millis() as f32)
                / 1000.0,
        );
        let mut list_state = ListState::default();
        list_state.select_first();
        self.app_state = AppState::Results(list_state)
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn new_word(&mut self) {
        self.current_word = random_word::get(self.lang);
        self.input.clear();
        self.wrong_input_chars.clear();
    }

    fn open_settings(&mut self) {
        self.temp_lang = self.lang;
        self.temp_limit = self.words_limit.to_string();
        self.app_state = AppState::Settings;
    }

    fn apply_settings(&mut self) {
        let new_limit = self
            .temp_limit
            .parse::<usize>()
            .map(|val| if val > 0 { val } else { self.words_limit })
            .unwrap_or(self.words_limit);

        if self.lang != self.temp_lang || self.words_limit != new_limit {
            self.lang = self.temp_lang;
            self.words_limit = new_limit;
            self.settings_changed = true;
        }
    }

    fn calculate_accuracy(&mut self) -> f32 {
        let total_typed_chars: usize = self.words.iter().map(|w| w.word.chars().count()).sum();
        let total_wrong_chars: usize = self.words.iter().map(|w| w.wrong_chars.len()).sum();

        if total_typed_chars == 0 {
            return 100.0;
        }

        let correct_chars = total_typed_chars - total_wrong_chars;

        (correct_chars as f32 / total_typed_chars as f32) * 100.0
    }
}

fn get_lang(lang: &str) -> Option<Lang> {
    match lang.to_uppercase().as_str() {
        "RU" => Some(Lang::Ru),
        "DE" => Some(Lang::De),
        "ES" => Some(Lang::Es),
        "FR" => Some(Lang::Fr),
        "JA" => Some(Lang::Ja),
        "ZH" => Some(Lang::Zh),
        "EN" => Some(Lang::En),
        _ => None,
    }
}

fn next_lang(lang: Lang) -> Lang {
    match lang {
        Lang::En => Lang::Ru,
        Lang::Ru => Lang::De,
        Lang::De => Lang::Es,
        Lang::Es => Lang::Fr,
        Lang::Fr => Lang::Ja,
        Lang::Ja => Lang::Zh,
        Lang::Zh => Lang::En,
    }
}

fn prev_lang(lang: Lang) -> Lang {
    match lang {
        Lang::En => Lang::Zh,
        Lang::Ru => Lang::En,
        Lang::De => Lang::Ru,
        Lang::Es => Lang::De,
        Lang::Fr => Lang::Es,
        Lang::Ja => Lang::Fr,
        Lang::Zh => Lang::Ja,
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    lang: String,
    limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lang: "EN".to_string(),
            limit: 50,
        }
    }
}

#[derive(Default)]
struct Word<'a> {
    word: &'a str,
    wrong_chars: HashSet<usize>,
}

impl<'a> From<&'a str> for Word<'a> {
    fn from(value: &'a str) -> Self {
        Self {
            word: value,
            wrong_chars: HashSet::new(),
        }
    }
}

const CONFIG: Lazy<Config> = Lazy::new(|| {
    get_config().unwrap_or_else(|err| {
        eprintln!("Failed to load config: {}. Using default.", err);
        Config::default()
    })
});

fn get_config() -> Result<Config, Box<dyn std::error::Error>> {
    if let Some(proj_dirs) = ProjectDirs::from("", "hdvtdev", "ktapper") {
        let config_dir = proj_dirs.config_dir();
        let config_file_path = config_dir.join("config.toml");

        if !config_dir.exists() {
            fs::create_dir_all(config_dir)?;
        }

        if config_file_path.exists() {
            let config_content = fs::read_to_string(config_file_path)?;
            let config: Config = toml::from_str(&config_content)?;
            Ok(config)
        } else {
            let default_config = Config::default();
            let config_content = toml::to_string(&default_config)?;

            let commented_config_content = format!(
                "{}\n# Limit range: 0 < limit <= usize\n# Available words languages: \"RU\" \"DE\" \"ES\" \"FR\" \"JA\" \"ZH\" \"EN\" \n# This will not affect the language of the interface.",
                config_content
            );
            fs::write(config_file_path, commented_config_content)?;
            Ok(default_config)
        }
    } else {
        Err("Could not find project directories".into())
    }
}

fn main() -> std::io::Result<()> {
    show()
}

fn show() -> std::io::Result<()> {
    let mut term = ratatui::init();
    
    if CONFIG.limit == 0 {
        return Ok(());
    }

    let mut app = App::from(&CONFIG);

    let result = run(&mut term, &mut app);
    ratatui::restore();

    result
}

fn run(term: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    while !app.exit {
        term.draw(|f| render(f, app))?;
        if let Event::Key(key) = event::read()? {
            match &mut app.app_state {
                AppState::Input => match key.code {
                    KeyCode::Esc => app.pause(),
                    KeyCode::Char(ch) => {
                        if app.start.is_none() {
                            app.start();
                        }

                        app.input.push(ch);
                        let input_len = app.input.chars().count();
                        let index = max(0, input_len as i32 - 1) as usize;

                        if app.current_word.char_at(index) != app.input.char_at(index) {
                            app.wrong_input_chars.insert(index);
                        }

                        if input_len >= app.current_word.chars().count() {
                            if !app.wrong_input_chars.is_empty() {
                                app.wrong_words.insert(app.words.len());
                            }

                            app.words.push(Word {
                                word: app.current_word,
                                wrong_chars: std::mem::take(&mut app.wrong_input_chars),
                            });

                            if app.words.len() >= app.words_limit {
                                app.finish();
                            } else {
                                app.new_word();
                            }
                        }
                    }
                    _ => {}
                },
                AppState::Pause(_) => {
                    match key.code {
                        KeyCode::Char('q') => app.exit(),
                        KeyCode::Char('s') => app.open_settings(),
                        _ => app.resume(), // Any key to resume
                    }
                }
                AppState::Results(list_state) => match key.code {
                    KeyCode::Up => list_state.select_previous(),
                    KeyCode::Down => list_state.select_next(),
                    KeyCode::Char('q') => app.exit(),
                    KeyCode::Char('r') => app.restart(),
                    KeyCode::Char('s') => app.open_settings(),
                    _ => {}
                },
                AppState::Settings => match key.code {
                    KeyCode::Esc => {
                        app.app_state = AppState::Input;
                        if app.settings_changed {
                            app.restart();
                            app.settings_changed = false;
                        }
                    }
                    KeyCode::Up | KeyCode::Down => {
                        app.selected_setting = if app.selected_setting == SelectedSetting::Lang {
                            SelectedSetting::Limit
                        } else {
                            SelectedSetting::Lang
                        };
                    }
                    KeyCode::Left => match app.selected_setting {
                        SelectedSetting::Lang => app.temp_lang = prev_lang(app.temp_lang),
                        SelectedSetting::Limit => {
                            let mut limit = app.temp_limit.parse().unwrap_or(1);
                            limit = max(1, limit - 1);
                            app.temp_limit = limit.to_string();
                        }
                    },
                    KeyCode::Right => match app.selected_setting {
                        SelectedSetting::Lang => app.temp_lang = next_lang(app.temp_lang),
                        SelectedSetting::Limit => {
                            let mut limit = app.temp_limit.parse().unwrap_or(0);
                            limit = min(u16::MAX as usize, limit.saturating_add(1));
                            app.temp_limit = limit.to_string();
                        }
                    },
                    KeyCode::Char(ch) if ch.is_digit(10) => {
                        if app.selected_setting == SelectedSetting::Limit {
                            app.temp_limit.push(ch);
                        }
                    }
                    KeyCode::Backspace => {
                        if app.selected_setting == SelectedSetting::Limit {
                            app.temp_limit.pop();
                        }
                    }
                    KeyCode::Enter => {
                        app.apply_settings();
                        app.app_state = AppState::Input;
                        if app.settings_changed {
                            app.restart();
                            app.settings_changed = false;
                        }
                    }
                    _ => {}
                },
            }
        }
    }

    Ok(())
}

fn render_settings(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(10),
            Constraint::Percentage(30),
        ])
        .split(area);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(50),
            Constraint::Percentage(30),
        ])
        .split(popup_layout[1])[1];

    let block = Block::default()
        .title("Settings")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(block, popup_area);

    let settings_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(popup_area);

    let lang_style = if app.selected_setting == SelectedSetting::Lang {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let limit_style = if app.selected_setting == SelectedSetting::Limit {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let lang_text = format!("< Left/Right > Language: {:?}", app.temp_lang);
    let limit_text = format!("< Left/Right > Words Limit (or type): {}", app.temp_limit);

    let lang_paragraph = Paragraph::new(lang_text).style(lang_style);
    let limit_paragraph = Paragraph::new(limit_text).style(limit_style);

    let help_text = Paragraph::new("Enter to save | Esc to discard").alignment(Alignment::Center);

    frame.render_widget(lang_paragraph, settings_layout[0]);
    frame.render_widget(limit_paragraph, settings_layout[1]);
    frame.render_widget(help_text, settings_layout[2]);
}

fn render(frame: &mut Frame, app: &mut App) {
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let is_settings = matches!(app.app_state, AppState::Settings);
    let accuracy = if matches!(&app.app_state, AppState::Results(_)) {
        Some(app.calculate_accuracy())
    } else {
        None
    };

    match &mut app.app_state {
        AppState::Input | AppState::Pause(_) | AppState::Settings => {
            let is_paused = matches!(app.app_state, AppState::Pause(_));

            let help_text = if is_paused {
                "Any key to resume | Q to Exit | S for settings"
            } else {
                "Press ESC to pause"
            };
            Line::from(help_text).render(vertical_chunks[5], frame.buffer_mut());

            #[cfg(debug_assertions)]
            {
                let debug_info = Paragraph::new(
                    app.wrong_input_chars
                        .iter()
                        .map(|f| f.to_string())
                        .collect::<Vec<String>>()
                        .join(" "),
                )
                .alignment(Alignment::Center);
                frame.render_widget(debug_info, vertical_chunks[0]);
            }

            let word_display = Paragraph::new(app.current_word)
                .alignment(Alignment::Center)
                .style(Style::new().add_modifier(Modifier::BOLD));
            frame.render_widget(word_display, vertical_chunks[1]);

            let styled_input = stylize(app.input.as_str(), &app.wrong_input_chars);
            let input_paragraph = Paragraph::new(Line::from(styled_input))
                .block(Block::default().borders(Borders::ALL).title(ternary!(
                    !is_paused,
                    format!("{}/{}", app.words.len(), app.words_limit),
                    "Paused".to_string()
                )))
                .alignment(Alignment::Center);
            frame.render_widget(input_paragraph, vertical_chunks[3]);

            if app.start.is_none() {
                let start_prompt = Paragraph::new("Enter any character to start")
                    .block(
                        Block::bordered()
                            .border_type(BorderType::Rounded)
                            .borders(Borders::ALL),
                    )
                    .alignment(Alignment::Center);
                frame.render_widget(start_prompt, vertical_chunks[4]);
            }
        }
        AppState::Results(list_state) => {
            Line::from("R Restart | Q Exit | S Settings")
                .render(vertical_chunks[5], frame.buffer_mut());

            let result_text = ternary!(
                app.wrong_words.is_empty(),
                format!(
                    "No mistakes, well done! Time elapsed: {}s",
                    app.finished_time.unwrap()
                ),
                format!(
                    "{} wrong typed words out of {}, Accuracy: {:.2}%, time elapsed: {}s",
                    app.wrong_words.len(),
                    app.words_limit,
                    accuracy.unwrap(),
                    app.finished_time.unwrap()
                )
            );

            let result_paragraph = Paragraph::new(Line::from(result_text))
                .block(Block::default().borders(Borders::ALL))
                .alignment(Alignment::Center);
            frame.render_widget(result_paragraph, vertical_chunks[3]);

            let list_items: Vec<Line> = app
                .words
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    let num = Span::raw(format!("{}. ", i + 1));
                    if !app.wrong_words.contains(&i) {
                        Line::from(vec![
                            num,
                            Span::styled(w.word, Style::new().fg(Color::Green)),
                        ])
                    } else {
                        let mut styled_word = stylize(w.word, &w.wrong_chars);
                        styled_word.insert(0, num);
                        Line::from(styled_word)
                    }
                })
                .collect();

            let list = List::new(list_items)
                .block(
                    Block::bordered()
                        .title("Results")
                        .border_type(BorderType::Rounded),
                )
                .highlight_symbol("> ")
                .highlight_style(Style::default().add_modifier(Modifier::BOLD));

            frame.render_stateful_widget(list, vertical_chunks[0], &mut list_state.to_owned());
        }
    }

    if is_settings {
        render_settings(frame, app);
    }
}

fn stylize<'a>(word: &str, wrong_chars: &HashSet<usize>) -> Vec<Span<'a>> {
    word.chars()
        .enumerate()
        .map(|(i, ch)| {
            let style = if wrong_chars.contains(&i) {
                Style::new().red()
            } else {
                Style::new().green()
            };
            Span::styled(ch.to_string(), style)
        })
        .collect()
}
