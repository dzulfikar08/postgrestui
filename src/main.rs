use app::{App, ConnectionConfig};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::{Backend, CrosstermBackend},
    restore, Terminal,
};
use std::io;
use ui::Ui;

pub mod api;
pub mod app;
pub mod ui;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// PostgreSQL hostname or IP
    #[arg(short = 's', long = "server", default_value = "localhost")]
    server: String,

    /// PostgreSQL port
    #[arg(short = 'p', long = "port", default_value_t = 5432)]
    port: u16,

    /// Database name
    #[arg(short = 'd', long = "database", default_value = "postgres")]
    database: String,

    /// Username
    #[arg(short = 'u', long = "user", default_value = "postgres")]
    user: String,

    /// Password
    #[arg(short = 'P', long = "pass", default_value = "")]
    password: String,

    /// Launch web UI instead of TUI
    #[arg(long = "ui", default_value_t = false)]
    web_ui: bool,

    /// Port for web UI server (default: 5000)
    #[arg(long = "listen", default_value_t = 5000)]
    listen: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    color_eyre::install()?;
    let cli = CliArgs::parse();

    let config = ConnectionConfig {
        host: cli.server.clone(),
        port: cli.port,
        database: cli.database.clone(),
        username: cli.user.clone(),
        password: cli.password.clone(),
    };

    if cli.web_ui {
        api::start_server(config, cli.listen).await?;
        return Ok(());
    }

    let mut app = App::default();
    let mut ui = Ui::new();

    app.connect(config).await?;
    if let Some(db) = &app.current_db {
        ui.table_view.load_nav(db);
    }

    enable_raw_mode()?;
    set_panic_hook();
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut app, &mut ui).await;

    if let Err(err) = res {
        eprintln!("{err}")
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    ui: &mut Ui,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| ui.ui(f, app))?;
        if let Event::Key(event) = event::read()? {
            if event.kind == event::KeyEventKind::Release {
                continue;
            }
            ui.handle_input(&event, app).await?;
            if event.code == KeyCode::Char('q')
                && event.modifiers.contains(event::KeyModifiers::CONTROL)
            {
                break;
            }
        }
    }
    Ok(())
}

fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore();
        hook(panic_info);
    }));
}
