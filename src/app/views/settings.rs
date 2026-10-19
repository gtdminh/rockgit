use cacao::appkit::window::{Window, WindowConfig};
use cacao::button::Button;
use cacao::color::Color;
use cacao::input::TextField;
use cacao::layout::{Layout, LayoutConstraint};
use cacao::select::Select;
use cacao::text::{Label, LineBreakMode};
use cacao::view::View;

use crate::app::{
    actions::AppAction,
    model::{AiConfig, AiProviderCli, AppConfig},
    send_ui,
};

const DIALOG_WIDTH: f64 = 520.;
const DIALOG_HEIGHT: f64 = 290.;
const FORM_LABEL_WIDTH: f64 = 140.;
const FIELD_HEIGHT: f64 = 28.;
const PROVIDER_OPTIONS: [(AiProviderCli, &str); 4] = [
    (AiProviderCli::OpenAi, "OpenAI CLI"),
    (AiProviderCli::Gemini, "Gemini CLI"),
    (AiProviderCli::Anthropic, "Anthropic CLI"),
    (AiProviderCli::Copilot, "Copilot CLI"),
];

pub struct SettingsDialog {
    window: Window,
    content: View,
    title: Label,
    autorefresh_label: Label,
    autorefresh_input: TextField,
    autorefresh_help: Label,
    provider_label: Label,
    provider_select: Select,
    cli_path_label: Label,
    cli_path_input: TextField,
    status: Label,
    cancel: Button,
    save: Button,
}

impl SettingsDialog {
    pub fn new() -> Self {
        let window = Window::new(WindowConfig::default());
        let content = View::new();

        let title = Label::new();
        title.set_text("Settings");

        let autorefresh_label = Label::new();
        autorefresh_label.set_text("Auto-refresh (sec)");

        let autorefresh_input = TextField::new();
        autorefresh_input.set_placeholder_text("0 disables background refresh");

        let autorefresh_help = Label::new();
        autorefresh_help.set_text("Set to 0 to disable periodic refreshes.");
        autorefresh_help.set_text_color(Color::LabelSecondary);
        autorefresh_help.set_max_number_of_lines(2);
        autorefresh_help.set_line_break_mode(LineBreakMode::WrapWords);

        let provider_label = Label::new();
        provider_label.set_text("AI provider CLI");

        let provider_select = Select::new();
        for (_, title) in PROVIDER_OPTIONS {
            provider_select.add_item(title);
        }

        let cli_path_label = Label::new();
        cli_path_label.set_text("CLI path");

        let cli_path_input = TextField::new();
        cli_path_input.set_placeholder_text("Optional custom executable path");

        let status = Label::new();
        status.set_text_color(Color::SystemRed);
        status.set_max_number_of_lines(2);
        status.set_line_break_mode(LineBreakMode::WrapWords);

        let mut cancel = Button::new("Cancel");
        cancel.set_action(|| send_ui(AppAction::CloseSettingsDialog));

        let mut save = Button::new("Save");
        save.set_action(|| send_ui(AppAction::SaveSettingsDialog));

        let dialog = Self {
            window,
            content,
            title,
            autorefresh_label,
            autorefresh_input,
            autorefresh_help,
            provider_label,
            provider_select,
            cli_path_label,
            cli_path_input,
            status,
            cancel,
            save,
        };

        dialog.build();
        dialog
    }

    pub fn show(&self, config: &AppConfig) {
        self.apply_config(config);
        self.clear_status();
        self.window.make_key_and_order_front();
    }

    pub fn close(&self) {
        self.window.close();
    }

    pub fn read_config(&self) -> Result<AppConfig, String> {
        let autorefresh_value = self.autorefresh_input.get_value();
        let autorefresh_value = autorefresh_value.trim();
        let autorefresh_time = if autorefresh_value.is_empty() {
            0
        } else {
            autorefresh_value.parse::<u64>().map_err(|error| {
                format!("Auto-refresh must be a whole number of seconds: {error}")
            })?
        };

        let provider_cli = provider_for_index(self.provider_select.get_selected_index())
            .ok_or_else(|| {
                "Choose a supported AI provider CLI before saving settings".to_owned()
            })?;

        Ok(AppConfig {
            autorefresh_time,
            ai: AiConfig {
                provider_cli,
                cli_path: self.cli_path_input.get_value().trim().to_owned(),
            },
        })
    }

    pub fn show_error(&self, message: &str) {
        self.status.set_text(message);
    }

    fn apply_config(&self, config: &AppConfig) {
        self.autorefresh_input
            .set_text(&config.autorefresh_time.to_string());
        self.provider_select
            .set_selected_index(provider_index(config.ai.provider_cli) as _);
        self.cli_path_input.set_text(&config.ai.cli_path);
    }

    fn clear_status(&self) {
        self.status.set_text("");
    }

    fn build(&self) {
        self.window.set_title("Settings");
        self.window.set_content_size(DIALOG_WIDTH, DIALOG_HEIGHT);
        self.window
            .set_minimum_content_size(DIALOG_WIDTH, DIALOG_HEIGHT);
        self.window
            .set_maximum_content_size(DIALOG_WIDTH, DIALOG_HEIGHT);
        self.window.set_excluded_from_windows_menu(true);
        self.window.set_content_view(&self.content);

        self.content.add_subview(&self.title);
        self.content.add_subview(&self.autorefresh_label);
        self.content.add_subview(&self.autorefresh_input);
        self.content.add_subview(&self.autorefresh_help);
        self.content.add_subview(&self.provider_label);
        self.content.add_subview(&self.provider_select);
        self.content.add_subview(&self.cli_path_label);
        self.content.add_subview(&self.cli_path_input);
        self.content.add_subview(&self.status);
        self.content.add_subview(&self.cancel);
        self.content.add_subview(&self.save);

        LayoutConstraint::activate(&[
            self.title
                .top
                .constraint_equal_to(&self.content.top)
                .offset(20.),
            self.title
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(20.),
            self.title
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-20.),
            self.autorefresh_label
                .top
                .constraint_equal_to(&self.title.bottom)
                .offset(20.),
            self.autorefresh_label
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(20.),
            self.autorefresh_label
                .width
                .constraint_equal_to_constant(FORM_LABEL_WIDTH),
            self.autorefresh_input
                .top
                .constraint_equal_to(&self.title.bottom)
                .offset(16.),
            self.autorefresh_input
                .leading
                .constraint_equal_to(&self.autorefresh_label.trailing)
                .offset(12.),
            self.autorefresh_input
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-20.),
            self.autorefresh_input
                .height
                .constraint_equal_to_constant(FIELD_HEIGHT),
            self.autorefresh_help
                .top
                .constraint_equal_to(&self.autorefresh_input.bottom)
                .offset(6.),
            self.autorefresh_help
                .leading
                .constraint_equal_to(&self.autorefresh_input.leading),
            self.autorefresh_help
                .trailing
                .constraint_equal_to(&self.autorefresh_input.trailing),
            self.provider_label
                .top
                .constraint_equal_to(&self.autorefresh_help.bottom)
                .offset(18.),
            self.provider_label
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(20.),
            self.provider_label
                .width
                .constraint_equal_to_constant(FORM_LABEL_WIDTH),
            self.provider_select
                .top
                .constraint_equal_to(&self.autorefresh_help.bottom)
                .offset(14.),
            self.provider_select
                .leading
                .constraint_equal_to(&self.provider_label.trailing)
                .offset(12.),
            self.provider_select
                .width
                .constraint_equal_to_constant(220.),
            self.cli_path_label
                .top
                .constraint_equal_to(&self.provider_select.bottom)
                .offset(18.),
            self.cli_path_label
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(20.),
            self.cli_path_label
                .width
                .constraint_equal_to_constant(FORM_LABEL_WIDTH),
            self.cli_path_input
                .top
                .constraint_equal_to(&self.provider_select.bottom)
                .offset(14.),
            self.cli_path_input
                .leading
                .constraint_equal_to(&self.cli_path_label.trailing)
                .offset(12.),
            self.cli_path_input
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-20.),
            self.cli_path_input
                .height
                .constraint_equal_to_constant(FIELD_HEIGHT),
            self.save
                .bottom
                .constraint_equal_to(&self.content.bottom)
                .offset(-20.),
            self.save
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-20.),
            self.cancel
                .bottom
                .constraint_equal_to(&self.content.bottom)
                .offset(-20.),
            self.cancel
                .trailing
                .constraint_equal_to(&self.save.leading)
                .offset(-8.),
            self.status
                .top
                .constraint_equal_to(&self.cli_path_input.bottom)
                .offset(16.),
            self.status
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(20.),
            self.status
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-20.),
            self.status
                .bottom
                .constraint_equal_to(&self.save.top)
                .offset(-14.),
        ]);
    }
}

fn provider_index(provider: AiProviderCli) -> usize {
    PROVIDER_OPTIONS
        .iter()
        .position(|(candidate, _)| *candidate == provider)
        .unwrap_or(0)
}

fn provider_for_index(index: usize) -> Option<AiProviderCli> {
    PROVIDER_OPTIONS.get(index).map(|(provider, _)| *provider)
}

#[cfg(test)]
mod tests;
