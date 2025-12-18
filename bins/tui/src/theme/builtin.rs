//! Built-in themes

use super::types::{BorderType, SerializableColor, Theme, ThemeColors, ThemeStyles};

impl Theme {
    /// Default theme - subtle and clean
    pub fn default() -> Self {
        Self {
            name: "default".to_string(),
            category: super::types::ThemeCategory::Dark,
            ui_text: super::types::UiText::default(),
            colors: ThemeColors {
                primary: SerializableColor::Named("cyan".to_string()),
                secondary: SerializableColor::Named("blue".to_string()),
                accent: SerializableColor::Named("magenta".to_string()),
                background: SerializableColor::Named("black".to_string()),
                text: SerializableColor::Named("white".to_string()),
                text_dim: SerializableColor::Named("gray".to_string()),
                success: SerializableColor::Named("green".to_string()),
                error: SerializableColor::Named("red".to_string()),
                warning: SerializableColor::Named("yellow".to_string()),
                prompt_border: SerializableColor::Named("cyan".to_string()),
                modal_overlay: SerializableColor::Named("black".to_string()),
                cursor: SerializableColor::Named("white".to_string()),
                selection: SerializableColor::Named("blue".to_string()),
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: false,
                spinners: super::types::SpinnerConfig::default(),
            },
            override_terminal_background: false,
        }
    }

    /// LCARS theme - inspired by Star Trek LCARS interface
    /// Colors from authentic LCARS palette: https://www.thelcars.com/colors.php
    pub fn lcars() -> Self {
        Self {
            name: "lcars".to_string(),
            category: super::types::ThemeCategory::Dark,
            ui_text: super::types::UiText {
                app_name: "KQL-PANOPTICON".to_string(),
                title_format: "{app} > {view}".to_string(),
                // Gold for app name
                title_app_color: Some(SerializableColor::Rgb {
                    r: 0xff,
                    g: 0xaa,
                    b: 0x00,
                }),
                // African-violet for separator
                title_separator_color: Some(SerializableColor::Rgb {
                    r: 0xcc,
                    g: 0x99,
                    b: 0xff,
                }),
                // Ice blue for view
                title_view_color: Some(SerializableColor::Rgb {
                    r: 0x99,
                    g: 0xcc,
                    b: 0xff,
                }),
            },
            colors: ThemeColors {
                // Butterscotch - classic LCARS orange
                primary: SerializableColor::Rgb {
                    r: 0xff,
                    g: 0x99,
                    b: 0x66,
                },
                // African-violet - signature LCARS purple
                secondary: SerializableColor::Rgb {
                    r: 0xcc,
                    g: 0x99,
                    b: 0xff,
                },
                // Ice - LCARS blue
                accent: SerializableColor::Rgb {
                    r: 0x99,
                    g: 0xcc,
                    b: 0xff,
                },
                // Black background (essential for LCARS)
                background: SerializableColor::Named("black".to_string()),
                // background: SerializableColor::Rgb {
                //     r: 0x0,
                //     g: 0x0,
                //     b: 0x0,
                // },
                // Space-white for text
                text: SerializableColor::Rgb {
                    r: 0xf5,
                    g: 0xf6,
                    b: 0xfa,
                },
                // Gray for dimmed text
                text_dim: SerializableColor::Rgb {
                    r: 0x66,
                    g: 0x66,
                    b: 0x88,
                },
                // Lima-bean - LCARS green/yellow
                success: SerializableColor::Rgb {
                    r: 0xcc,
                    g: 0xcc,
                    b: 0x66,
                },
                // Tomato - LCARS red
                error: SerializableColor::Rgb {
                    r: 0xff,
                    g: 0x55,
                    b: 0x55,
                },
                // Sunflower - LCARS amber/warning
                warning: SerializableColor::Rgb {
                    r: 0xff,
                    g: 0xcc,
                    b: 0x99,
                },
                // African-violet for prompt border
                prompt_border: SerializableColor::Rgb {
                    r: 0xcc,
                    g: 0x99,
                    b: 0xff,
                },
                // Black overlay
                modal_overlay: SerializableColor::Named("black".to_string()),
                // Gold cursor
                cursor: SerializableColor::Rgb {
                    r: 0xff,
                    g: 0xaa,
                    b: 0x00,
                },
                // Muted butterscotch for selection (easier on eyes)
                selection: SerializableColor::Rgb {
                    r: 180,
                    g: 120,
                    b: 60,
                },
                // Gold for command prefix
                command_prefix: SerializableColor::Rgb {
                    r: 0xff,
                    g: 0xaa,
                    b: 0x00,
                },
                // Ice for command name
                command_name: SerializableColor::Rgb {
                    r: 0x99,
                    g: 0xcc,
                    b: 0xff,
                },
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: true,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 120,
                    running: super::types::SpinnerFrames {
                        // LCARS-style horizontal scanner animation
                        frames: vec![
                            "▏".to_string(),
                            "▎".to_string(),
                            "▍".to_string(),
                            "▌".to_string(),
                            "▋".to_string(),
                            "▊".to_string(),
                            "▉".to_string(),
                            "█".to_string(),
                            "▉".to_string(),
                            "▊".to_string(),
                            "▋".to_string(),
                            "▌".to_string(),
                            "▍".to_string(),
                            "▎".to_string(),
                        ],
                        // Cycle through LCARS colors: butterscotch → ice → violet
                        colors: vec![
                            Some(SerializableColor::Rgb { r: 0xff, g: 0x99, b: 0x66 }), // Butterscotch
                            Some(SerializableColor::Rgb { r: 0xff, g: 0xaa, b: 0x77 }),
                            Some(SerializableColor::Rgb { r: 0xdd, g: 0xaa, b: 0x99 }),
                            Some(SerializableColor::Rgb { r: 0xbb, g: 0xaa, b: 0xcc }),
                            Some(SerializableColor::Rgb { r: 0x99, g: 0xaa, b: 0xee }),
                            Some(SerializableColor::Rgb { r: 0x99, g: 0xcc, b: 0xff }), // Ice
                            Some(SerializableColor::Rgb { r: 0xaa, g: 0xbb, b: 0xff }),
                            Some(SerializableColor::Rgb { r: 0xcc, g: 0x99, b: 0xff }), // Violet
                            Some(SerializableColor::Rgb { r: 0xaa, g: 0xbb, b: 0xff }),
                            Some(SerializableColor::Rgb { r: 0x99, g: 0xcc, b: 0xff }), // Ice
                            Some(SerializableColor::Rgb { r: 0x99, g: 0xaa, b: 0xee }),
                            Some(SerializableColor::Rgb { r: 0xbb, g: 0xaa, b: 0xcc }),
                            Some(SerializableColor::Rgb { r: 0xdd, g: 0xaa, b: 0x99 }),
                            Some(SerializableColor::Rgb { r: 0xff, g: 0xaa, b: 0x77 }),
                        ],
                    },
                    completed: "▣".to_string(), // LCARS-style filled square
                    failed: "▢".to_string(),    // Hollow square
                },
            },
            override_terminal_background: true,
        }
    }

    /// Cyberpunk theme - neon colors with fast pulsing animation
    pub fn cyberpunk() -> Self {
        Self {
            name: "cyberpunk".to_string(),
            category: super::types::ThemeCategory::Dark,
            ui_text: super::types::UiText {
                app_name: "kql-panopticon".to_string(),
                title_format: "{app} :: {view}".to_string(),
                title_app_color: Some(SerializableColor::Rgb {
                    r: 255,
                    g: 0,
                    b: 255,
                }), // Hot pink
                title_separator_color: Some(SerializableColor::Rgb {
                    r: 0,
                    g: 255,
                    b: 255,
                }), // Cyan
                title_view_color: Some(SerializableColor::Rgb {
                    r: 255,
                    g: 255,
                    b: 0,
                }), // Yellow
            },
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 255,
                    g: 0,
                    b: 255,
                }, // Hot pink
                secondary: SerializableColor::Rgb {
                    r: 0,
                    g: 255,
                    b: 255,
                }, // Cyan
                accent: SerializableColor::Rgb {
                    r: 255,
                    g: 255,
                    b: 0,
                }, // Yellow
                background: SerializableColor::Rgb { r: 10, g: 0, b: 20 }, // Dark purple
                text: SerializableColor::Rgb {
                    r: 0,
                    g: 255,
                    b: 200,
                }, // Bright cyan
                text_dim: SerializableColor::Rgb {
                    r: 100,
                    g: 100,
                    b: 150,
                }, // Muted purple
                success: SerializableColor::Rgb {
                    r: 0,
                    g: 255,
                    b: 100,
                }, // Neon green
                error: SerializableColor::Rgb {
                    r: 255,
                    g: 0,
                    b: 100,
                }, // Hot pink
                warning: SerializableColor::Rgb {
                    r: 255,
                    g: 200,
                    b: 0,
                }, // Amber
                prompt_border: SerializableColor::Rgb {
                    r: 255,
                    g: 0,
                    b: 255,
                },
                modal_overlay: SerializableColor::Rgb { r: 10, g: 0, b: 20 },
                cursor: SerializableColor::Rgb {
                    r: 0,
                    g: 255,
                    b: 255,
                },
                selection: SerializableColor::Rgb {
                    r: 255,
                    g: 0,
                    b: 255,
                },
                // Hot pink for command prefix
                command_prefix: SerializableColor::Rgb {
                    r: 255,
                    g: 20,
                    b: 147,
                },
                // Cyan for command name
                command_name: SerializableColor::Rgb {
                    r: 0,
                    g: 255,
                    b: 255,
                },
            },
            styles: ThemeStyles {
                border_type: BorderType::Double,
                bold_borders: true,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 80, // Fast animation
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "⣾".to_string(),
                            "⣽".to_string(),
                            "⣻".to_string(),
                            "⢿".to_string(),
                            "⡿".to_string(),
                            "⣟".to_string(),
                            "⣯".to_string(),
                            "⣷".to_string(),
                        ],
                        colors: vec![
                            Some(SerializableColor::Rgb {
                                r: 255,
                                g: 0,
                                b: 255,
                            }),
                            Some(SerializableColor::Rgb {
                                r: 200,
                                g: 0,
                                b: 255,
                            }),
                            Some(SerializableColor::Rgb {
                                r: 100,
                                g: 100,
                                b: 255,
                            }),
                            Some(SerializableColor::Rgb {
                                r: 0,
                                g: 200,
                                b: 255,
                            }),
                            Some(SerializableColor::Rgb {
                                r: 0,
                                g: 255,
                                b: 255,
                            }),
                            Some(SerializableColor::Rgb {
                                r: 0,
                                g: 255,
                                b: 200,
                            }),
                            Some(SerializableColor::Rgb {
                                r: 100,
                                g: 255,
                                b: 100,
                            }),
                            Some(SerializableColor::Rgb {
                                r: 200,
                                g: 255,
                                b: 0,
                            }),
                        ],
                    },
                    completed: "◉".to_string(),
                    failed: "◈".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Solarized Dark theme - elegant blues and earth tones
    pub fn solarized_dark() -> Self {
        Self {
            name: "solarized-dark".to_string(),
            category: super::types::ThemeCategory::Dark,
            ui_text: super::types::UiText {
                app_name: "kql-panopticon".to_string(),
                title_format: "{app} > {view}".to_string(),
                title_app_color: None,
                title_separator_color: None,
                title_view_color: None,
            },
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 38,
                    g: 139,
                    b: 210,
                }, // Blue
                secondary: SerializableColor::Rgb {
                    r: 108,
                    g: 113,
                    b: 196,
                }, // Violet
                accent: SerializableColor::Rgb {
                    r: 42,
                    g: 161,
                    b: 152,
                }, // Cyan
                background: SerializableColor::Rgb { r: 0, g: 43, b: 54 }, // Base03
                text: SerializableColor::Rgb {
                    r: 131,
                    g: 148,
                    b: 150,
                }, // Base0
                text_dim: SerializableColor::Rgb {
                    r: 88,
                    g: 110,
                    b: 117,
                }, // Base01
                success: SerializableColor::Rgb {
                    r: 133,
                    g: 153,
                    b: 0,
                }, // Green
                error: SerializableColor::Rgb {
                    r: 220,
                    g: 50,
                    b: 47,
                }, // Red
                warning: SerializableColor::Rgb {
                    r: 181,
                    g: 137,
                    b: 0,
                }, // Yellow
                prompt_border: SerializableColor::Rgb {
                    r: 38,
                    g: 139,
                    b: 210,
                },
                modal_overlay: SerializableColor::Rgb { r: 0, g: 43, b: 54 },
                cursor: SerializableColor::Rgb {
                    r: 147,
                    g: 161,
                    b: 161,
                },
                selection: SerializableColor::Rgb { r: 7, g: 54, b: 66 },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: false,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 120, // Smooth, calm animation
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "◐".to_string(),
                            "◓".to_string(),
                            "◑".to_string(),
                            "◒".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "●".to_string(),
                    failed: "○".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Matrix theme - green on black with digital rain effect
    pub fn matrix() -> Self {
        Self {
            name: "matrix".to_string(),
            category: super::types::ThemeCategory::Dark,
            ui_text: super::types::UiText {
                app_name: "kql-panopticon".to_string(),
                title_format: "{app} | {view}".to_string(),
                title_app_color: Some(SerializableColor::Rgb { r: 0, g: 255, b: 0 }), // Bright green
                title_separator_color: Some(SerializableColor::Rgb { r: 0, g: 150, b: 0 }), // Dark green
                title_view_color: Some(SerializableColor::Rgb {
                    r: 100,
                    g: 255,
                    b: 100,
                }), // Light green
            },
            colors: ThemeColors {
                primary: SerializableColor::Rgb { r: 0, g: 255, b: 0 }, // Bright green
                secondary: SerializableColor::Rgb { r: 0, g: 200, b: 0 }, // Medium green
                accent: SerializableColor::Rgb {
                    r: 100,
                    g: 255,
                    b: 100,
                }, // Light green
                background: SerializableColor::Named("black".to_string()),
                text: SerializableColor::Rgb { r: 0, g: 200, b: 0 },
                text_dim: SerializableColor::Rgb { r: 0, g: 100, b: 0 },
                success: SerializableColor::Rgb { r: 0, g: 255, b: 0 },
                error: SerializableColor::Rgb { r: 255, g: 0, b: 0 },
                warning: SerializableColor::Rgb {
                    r: 200,
                    g: 200,
                    b: 0,
                },
                prompt_border: SerializableColor::Rgb { r: 0, g: 255, b: 0 },
                modal_overlay: SerializableColor::Named("black".to_string()),
                cursor: SerializableColor::Rgb { r: 0, g: 255, b: 0 },
                // Darker green for better text contrast
                selection: SerializableColor::Rgb { r: 0, g: 60, b: 0 },
                // Bright green for command prefix
                command_prefix: SerializableColor::Rgb { r: 0, g: 255, b: 0 },
                // Lime for command name
                command_name: SerializableColor::Rgb {
                    r: 50,
                    g: 255,
                    b: 50,
                },
            },
            styles: ThemeStyles {
                border_type: BorderType::Square,
                bold_borders: true,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 150, // Slow drip effect
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "ʌ".to_string(),
                            "v".to_string(),
                            "∨".to_string(),
                            "˅".to_string(),
                            "∨".to_string(),
                            "v".to_string(),
                        ],
                        colors: vec![
                            Some(SerializableColor::Rgb { r: 0, g: 255, b: 0 }),
                            Some(SerializableColor::Rgb { r: 0, g: 230, b: 0 }),
                            Some(SerializableColor::Rgb { r: 0, g: 200, b: 0 }),
                            Some(SerializableColor::Rgb { r: 0, g: 170, b: 0 }),
                            Some(SerializableColor::Rgb { r: 0, g: 140, b: 0 }),
                            Some(SerializableColor::Rgb { r: 0, g: 110, b: 0 }),
                        ],
                    },
                    completed: "█".to_string(),
                    failed: "▓".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Dracula theme - purple, pink, and cyan pastels
    pub fn dracula() -> Self {
        Self {
            name: "dracula".to_string(),
            category: super::types::ThemeCategory::Dark,
            ui_text: super::types::UiText {
                app_name: "kql-panopticon".to_string(),
                title_format: "{app} ~ {view}".to_string(),
                title_app_color: Some(SerializableColor::Rgb {
                    r: 189,
                    g: 147,
                    b: 249,
                }), // Purple
                title_separator_color: Some(SerializableColor::Rgb {
                    r: 255,
                    g: 121,
                    b: 198,
                }), // Pink
                title_view_color: Some(SerializableColor::Rgb {
                    r: 139,
                    g: 233,
                    b: 253,
                }), // Cyan
            },
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 189,
                    g: 147,
                    b: 249,
                }, // Purple
                secondary: SerializableColor::Rgb {
                    r: 255,
                    g: 121,
                    b: 198,
                }, // Pink
                accent: SerializableColor::Rgb {
                    r: 139,
                    g: 233,
                    b: 253,
                }, // Cyan
                background: SerializableColor::Rgb {
                    r: 40,
                    g: 42,
                    b: 54,
                }, // Dark
                text: SerializableColor::Rgb {
                    r: 248,
                    g: 248,
                    b: 242,
                }, // Foreground
                text_dim: SerializableColor::Rgb {
                    r: 98,
                    g: 114,
                    b: 164,
                }, // Comment
                success: SerializableColor::Rgb {
                    r: 80,
                    g: 250,
                    b: 123,
                }, // Green
                error: SerializableColor::Rgb {
                    r: 255,
                    g: 85,
                    b: 85,
                }, // Red
                warning: SerializableColor::Rgb {
                    r: 241,
                    g: 250,
                    b: 140,
                }, // Yellow
                prompt_border: SerializableColor::Rgb {
                    r: 189,
                    g: 147,
                    b: 249,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 40,
                    g: 42,
                    b: 54,
                },
                cursor: SerializableColor::Rgb {
                    r: 255,
                    g: 121,
                    b: 198,
                },
                selection: SerializableColor::Rgb {
                    r: 68,
                    g: 71,
                    b: 90,
                },
                // Purple for command prefix
                command_prefix: SerializableColor::Rgb {
                    r: 189,
                    g: 147,
                    b: 249,
                },
                // Pink for command name
                command_name: SerializableColor::Rgb {
                    r: 255,
                    g: 121,
                    b: 198,
                },
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: false,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 100,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "◜".to_string(),
                            "◠".to_string(),
                            "◝".to_string(),
                            "◞".to_string(),
                            "◡".to_string(),
                            "◟".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "✓".to_string(),
                    failed: "✗".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Gruvbox theme - warm retro earth tones
    pub fn gruvbox() -> Self {
        Self {
            name: "gruvbox".to_string(),
            category: super::types::ThemeCategory::Dark,
            ui_text: super::types::UiText {
                app_name: "kql-panopticon".to_string(),
                title_format: "{app} | {view}".to_string(),
                title_app_color: None,
                title_separator_color: None,
                title_view_color: None,
            },
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 215,
                    g: 153,
                    b: 33,
                }, // Orange
                secondary: SerializableColor::Rgb {
                    r: 177,
                    g: 98,
                    b: 134,
                }, // Purple
                accent: SerializableColor::Rgb {
                    r: 142,
                    g: 192,
                    b: 124,
                }, // Aqua
                background: SerializableColor::Rgb {
                    r: 40,
                    g: 40,
                    b: 40,
                }, // BG
                text: SerializableColor::Rgb {
                    r: 235,
                    g: 219,
                    b: 178,
                }, // FG
                text_dim: SerializableColor::Rgb {
                    r: 146,
                    g: 131,
                    b: 116,
                }, // FG4
                success: SerializableColor::Rgb {
                    r: 184,
                    g: 187,
                    b: 38,
                }, // Green
                error: SerializableColor::Rgb {
                    r: 251,
                    g: 73,
                    b: 52,
                }, // Red
                warning: SerializableColor::Rgb {
                    r: 250,
                    g: 189,
                    b: 47,
                }, // Yellow
                prompt_border: SerializableColor::Rgb {
                    r: 215,
                    g: 153,
                    b: 33,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 40,
                    g: 40,
                    b: 40,
                },
                cursor: SerializableColor::Rgb {
                    r: 251,
                    g: 241,
                    b: 199,
                },
                selection: SerializableColor::Rgb {
                    r: 60,
                    g: 56,
                    b: 54,
                },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Thick,
                bold_borders: true,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 140,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "←".to_string(),
                            "↖".to_string(),
                            "↑".to_string(),
                            "↗".to_string(),
                            "→".to_string(),
                            "↘".to_string(),
                            "↓".to_string(),
                            "↙".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "✓".to_string(),
                    failed: "✗".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Solarized Light theme - elegant light theme with warm tones
    pub fn solarized_light() -> Self {
        Self {
            name: "solarized-light".to_string(),
            category: super::types::ThemeCategory::Light,
            ui_text: super::types::UiText {
                app_name: "kql-panopticon".to_string(),
                title_format: "{app} > {view}".to_string(),
                title_app_color: None,
                title_separator_color: None,
                title_view_color: None,
            },
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 38,
                    g: 139,
                    b: 210,
                }, // Blue
                secondary: SerializableColor::Rgb {
                    r: 108,
                    g: 113,
                    b: 196,
                }, // Violet
                accent: SerializableColor::Rgb {
                    r: 42,
                    g: 161,
                    b: 152,
                }, // Cyan
                background: SerializableColor::Rgb {
                    r: 253,
                    g: 246,
                    b: 227,
                }, // Base3
                text: SerializableColor::Rgb {
                    r: 101,
                    g: 123,
                    b: 131,
                }, // Base00
                text_dim: SerializableColor::Rgb {
                    r: 147,
                    g: 161,
                    b: 161,
                }, // Base1
                success: SerializableColor::Rgb {
                    r: 133,
                    g: 153,
                    b: 0,
                }, // Green
                error: SerializableColor::Rgb {
                    r: 220,
                    g: 50,
                    b: 47,
                }, // Red
                warning: SerializableColor::Rgb {
                    r: 181,
                    g: 137,
                    b: 0,
                }, // Yellow
                prompt_border: SerializableColor::Rgb {
                    r: 38,
                    g: 139,
                    b: 210,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 238,
                    g: 232,
                    b: 213,
                },
                cursor: SerializableColor::Rgb {
                    r: 88,
                    g: 110,
                    b: 117,
                },
                selection: SerializableColor::Rgb {
                    r: 238,
                    g: 232,
                    b: 213,
                },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: false,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 120,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "◐".to_string(),
                            "◓".to_string(),
                            "◑".to_string(),
                            "◒".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "●".to_string(),
                    failed: "○".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Gruvbox Light theme - warm retro earth tones on light background
    pub fn gruvbox_light() -> Self {
        Self {
            name: "gruvbox-light".to_string(),
            category: super::types::ThemeCategory::Light,
            ui_text: super::types::UiText {
                app_name: "kql-panopticon".to_string(),
                title_format: "{app} | {view}".to_string(),
                title_app_color: None,
                title_separator_color: None,
                title_view_color: None,
            },
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 175,
                    g: 58,
                    b: 3,
                }, // Orange
                secondary: SerializableColor::Rgb {
                    r: 143,
                    g: 63,
                    b: 113,
                }, // Purple
                accent: SerializableColor::Rgb {
                    r: 66,
                    g: 123,
                    b: 88,
                }, // Aqua
                background: SerializableColor::Rgb {
                    r: 251,
                    g: 241,
                    b: 199,
                }, // BG
                text: SerializableColor::Rgb {
                    r: 60,
                    g: 56,
                    b: 54,
                }, // FG
                text_dim: SerializableColor::Rgb {
                    r: 146,
                    g: 131,
                    b: 116,
                }, // FG4
                success: SerializableColor::Rgb {
                    r: 121,
                    g: 116,
                    b: 14,
                }, // Green
                error: SerializableColor::Rgb {
                    r: 204,
                    g: 36,
                    b: 29,
                }, // Red
                warning: SerializableColor::Rgb {
                    r: 215,
                    g: 153,
                    b: 33,
                }, // Yellow
                prompt_border: SerializableColor::Rgb {
                    r: 175,
                    g: 58,
                    b: 3,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 235,
                    g: 219,
                    b: 178,
                },
                cursor: SerializableColor::Rgb {
                    r: 60,
                    g: 56,
                    b: 54,
                },
                // Lighter selection for better text contrast
                selection: SerializableColor::Rgb {
                    r: 230,
                    g: 215,
                    b: 185,
                },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Thick,
                bold_borders: true,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 140,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "←".to_string(),
                            "↖".to_string(),
                            "↑".to_string(),
                            "↗".to_string(),
                            "→".to_string(),
                            "↘".to_string(),
                            "↓".to_string(),
                            "↙".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "✓".to_string(),
                    failed: "✗".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// GitHub Light theme - clean, professional white/gray scheme
    pub fn github_light() -> Self {
        Self {
            name: "github-light".to_string(),
            category: super::types::ThemeCategory::Light,
            ui_text: super::types::UiText {
                app_name: "kql-panopticon".to_string(),
                title_format: "{app} - {view}".to_string(),
                title_app_color: None,
                title_separator_color: None,
                title_view_color: None,
            },
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 9,
                    g: 105,
                    b: 218,
                }, // Blue
                secondary: SerializableColor::Rgb {
                    r: 87,
                    g: 96,
                    b: 106,
                }, // Gray
                accent: SerializableColor::Rgb {
                    r: 106,
                    g: 115,
                    b: 125,
                }, // Light gray
                background: SerializableColor::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                }, // White
                text: SerializableColor::Rgb {
                    r: 36,
                    g: 41,
                    b: 47,
                }, // Dark gray
                text_dim: SerializableColor::Rgb {
                    r: 87,
                    g: 96,
                    b: 106,
                }, // Medium gray
                success: SerializableColor::Rgb {
                    r: 26,
                    g: 127,
                    b: 55,
                }, // Green
                error: SerializableColor::Rgb {
                    r: 207,
                    g: 34,
                    b: 46,
                }, // Red
                warning: SerializableColor::Rgb {
                    r: 158,
                    g: 106,
                    b: 3,
                }, // Yellow
                prompt_border: SerializableColor::Rgb {
                    r: 9,
                    g: 105,
                    b: 218,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 246,
                    g: 248,
                    b: 250,
                },
                cursor: SerializableColor::Rgb {
                    r: 36,
                    g: 41,
                    b: 47,
                },
                selection: SerializableColor::Rgb {
                    r: 208,
                    g: 215,
                    b: 222,
                },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: false,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 100,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "⠋".to_string(),
                            "⠙".to_string(),
                            "⠹".to_string(),
                            "⠸".to_string(),
                            "⠼".to_string(),
                            "⠴".to_string(),
                            "⠦".to_string(),
                            "⠧".to_string(),
                            "⠇".to_string(),
                            "⠏".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "✓".to_string(),
                    failed: "✗".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Catppuccin Latte theme - soft pastel colors on warm latte background
    pub fn catppuccin_latte() -> Self {
        Self {
            name: "catppuccin-latte".to_string(),
            category: super::types::ThemeCategory::Light,
            ui_text: super::types::UiText::default(),
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 30,
                    g: 102,
                    b: 245,
                }, // Blue
                secondary: SerializableColor::Rgb {
                    r: 136,
                    g: 57,
                    b: 239,
                }, // Mauve
                accent: SerializableColor::Rgb {
                    r: 23,
                    g: 146,
                    b: 153,
                }, // Teal
                background: SerializableColor::Rgb {
                    r: 239,
                    g: 241,
                    b: 245,
                }, // Base
                text: SerializableColor::Rgb {
                    r: 76,
                    g: 79,
                    b: 105,
                }, // Text
                text_dim: SerializableColor::Rgb {
                    r: 140,
                    g: 143,
                    b: 161,
                }, // Subtext0
                success: SerializableColor::Rgb {
                    r: 64,
                    g: 160,
                    b: 43,
                }, // Green
                error: SerializableColor::Rgb {
                    r: 210,
                    g: 15,
                    b: 57,
                }, // Red
                warning: SerializableColor::Rgb {
                    r: 223,
                    g: 142,
                    b: 29,
                }, // Yellow
                prompt_border: SerializableColor::Rgb {
                    r: 30,
                    g: 102,
                    b: 245,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 230,
                    g: 233,
                    b: 239,
                },
                cursor: SerializableColor::Rgb {
                    r: 220,
                    g: 138,
                    b: 120,
                },
                selection: SerializableColor::Rgb {
                    r: 220,
                    g: 224,
                    b: 232,
                },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: false,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 110,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "◜".to_string(),
                            "◠".to_string(),
                            "◝".to_string(),
                            "◞".to_string(),
                            "◡".to_string(),
                            "◟".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "✓".to_string(),
                    failed: "✗".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Nord Light theme - cool, minimal light theme with Arctic-inspired blues
    pub fn nord_light() -> Self {
        Self {
            name: "nord-light".to_string(),
            category: super::types::ThemeCategory::Light,
            ui_text: super::types::UiText::default(),
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 94,
                    g: 129,
                    b: 172,
                }, // Frost blue
                secondary: SerializableColor::Rgb {
                    r: 136,
                    g: 192,
                    b: 208,
                }, // Frost cyan
                accent: SerializableColor::Rgb {
                    r: 143,
                    g: 188,
                    b: 187,
                }, // Frost teal
                background: SerializableColor::Rgb {
                    r: 236,
                    g: 239,
                    b: 244,
                }, // Snow storm 1
                text: SerializableColor::Rgb {
                    r: 46,
                    g: 52,
                    b: 64,
                }, // Polar night 1
                text_dim: SerializableColor::Rgb {
                    r: 76,
                    g: 86,
                    b: 106,
                }, // Polar night 3
                success: SerializableColor::Rgb {
                    r: 163,
                    g: 190,
                    b: 140,
                }, // Aurora green
                error: SerializableColor::Rgb {
                    r: 191,
                    g: 97,
                    b: 106,
                }, // Aurora red
                warning: SerializableColor::Rgb {
                    r: 235,
                    g: 203,
                    b: 139,
                }, // Aurora yellow
                prompt_border: SerializableColor::Rgb {
                    r: 94,
                    g: 129,
                    b: 172,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 229,
                    g: 233,
                    b: 240,
                },
                cursor: SerializableColor::Rgb {
                    r: 59,
                    g: 66,
                    b: 82,
                },
                // More saturated blue for better visual definition
                selection: SerializableColor::Rgb {
                    r: 190,
                    g: 210,
                    b: 235,
                },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: false,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 130,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "⠁".to_string(),
                            "⠂".to_string(),
                            "⠄".to_string(),
                            "⡀".to_string(),
                            "⢀".to_string(),
                            "⠠".to_string(),
                            "⠐".to_string(),
                            "⠈".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "✓".to_string(),
                    failed: "✗".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// One Light theme - Atom editor's popular light theme
    pub fn one_light() -> Self {
        Self {
            name: "one-light".to_string(),
            category: super::types::ThemeCategory::Light,
            ui_text: super::types::UiText::default(),
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 64,
                    g: 120,
                    b: 242,
                }, // Blue
                secondary: SerializableColor::Rgb {
                    r: 166,
                    g: 38,
                    b: 164,
                }, // Purple
                accent: SerializableColor::Rgb {
                    r: 1,
                    g: 132,
                    b: 188,
                }, // Cyan
                background: SerializableColor::Rgb {
                    r: 250,
                    g: 250,
                    b: 250,
                }, // White
                text: SerializableColor::Rgb {
                    r: 56,
                    g: 58,
                    b: 66,
                }, // Mono 1
                text_dim: SerializableColor::Rgb {
                    r: 160,
                    g: 161,
                    b: 167,
                }, // Mono 3
                success: SerializableColor::Rgb {
                    r: 80,
                    g: 161,
                    b: 79,
                }, // Green
                error: SerializableColor::Rgb {
                    r: 228,
                    g: 86,
                    b: 73,
                }, // Red
                warning: SerializableColor::Rgb {
                    r: 193,
                    g: 132,
                    b: 1,
                }, // Orange
                prompt_border: SerializableColor::Rgb {
                    r: 64,
                    g: 120,
                    b: 242,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 240,
                    g: 240,
                    b: 240,
                },
                cursor: SerializableColor::Rgb {
                    r: 56,
                    g: 58,
                    b: 66,
                },
                selection: SerializableColor::Rgb {
                    r: 228,
                    g: 229,
                    b: 231,
                },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: false,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 100,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "⠋".to_string(),
                            "⠙".to_string(),
                            "⠹".to_string(),
                            "⠸".to_string(),
                            "⠼".to_string(),
                            "⠴".to_string(),
                            "⠦".to_string(),
                            "⠧".to_string(),
                            "⠇".to_string(),
                            "⠏".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "✓".to_string(),
                    failed: "✗".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Paper theme - minimalist white background with black text
    pub fn paper() -> Self {
        Self {
            name: "paper".to_string(),
            category: super::types::ThemeCategory::Light,
            ui_text: super::types::UiText {
                app_name: "kql-panopticon".to_string(),
                title_format: "{app} - {view}".to_string(),
                title_app_color: None,
                title_separator_color: None,
                title_view_color: None,
            },
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 70,
                    g: 130,
                    b: 180,
                }, // Steel blue
                secondary: SerializableColor::Rgb {
                    r: 105,
                    g: 105,
                    b: 105,
                }, // Dim gray
                accent: SerializableColor::Rgb {
                    r: 95,
                    g: 158,
                    b: 160,
                }, // Cadet blue
                background: SerializableColor::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                }, // Pure white
                text: SerializableColor::Rgb { r: 0, g: 0, b: 0 }, // Pure black
                text_dim: SerializableColor::Rgb {
                    r: 119,
                    g: 119,
                    b: 119,
                }, // Gray
                success: SerializableColor::Rgb {
                    r: 60,
                    g: 179,
                    b: 113,
                }, // Sea green
                error: SerializableColor::Rgb {
                    r: 220,
                    g: 20,
                    b: 60,
                }, // Crimson
                warning: SerializableColor::Rgb {
                    r: 218,
                    g: 165,
                    b: 32,
                }, // Goldenrod
                prompt_border: SerializableColor::Rgb {
                    r: 70,
                    g: 130,
                    b: 180,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 245,
                    g: 245,
                    b: 245,
                },
                cursor: SerializableColor::Rgb { r: 0, g: 0, b: 0 },
                // Slight blue tint for better visual definition
                selection: SerializableColor::Rgb {
                    r: 195,
                    g: 210,
                    b: 225,
                },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Square,
                bold_borders: false,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 150,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "—".to_string(),
                            "\\".to_string(),
                            "|".to_string(),
                            "/".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "✓".to_string(),
                    failed: "✗".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Sepia theme - easy on the eyes with warm paper-like beige tones
    pub fn sepia() -> Self {
        Self {
            name: "sepia".to_string(),
            category: super::types::ThemeCategory::Light,
            ui_text: super::types::UiText::default(),
            colors: ThemeColors {
                primary: SerializableColor::Rgb {
                    r: 139,
                    g: 90,
                    b: 43,
                }, // Brown
                secondary: SerializableColor::Rgb {
                    r: 160,
                    g: 82,
                    b: 45,
                }, // Sienna
                accent: SerializableColor::Rgb {
                    r: 188,
                    g: 143,
                    b: 143,
                }, // Rosy brown
                background: SerializableColor::Rgb {
                    r: 245,
                    g: 235,
                    b: 220,
                }, // Sepia background
                text: SerializableColor::Rgb {
                    r: 74,
                    g: 54,
                    b: 33,
                }, // Dark brown
                text_dim: SerializableColor::Rgb {
                    r: 139,
                    g: 115,
                    b: 85,
                }, // Medium brown
                success: SerializableColor::Rgb {
                    r: 107,
                    g: 142,
                    b: 35,
                }, // Olive green
                error: SerializableColor::Rgb {
                    r: 178,
                    g: 34,
                    b: 34,
                }, // Fire brick
                warning: SerializableColor::Rgb {
                    r: 184,
                    g: 134,
                    b: 11,
                }, // Dark goldenrod
                prompt_border: SerializableColor::Rgb {
                    r: 139,
                    g: 90,
                    b: 43,
                },
                modal_overlay: SerializableColor::Rgb {
                    r: 230,
                    g: 220,
                    b: 205,
                },
                cursor: SerializableColor::Rgb {
                    r: 74,
                    g: 54,
                    b: 33,
                },
                selection: SerializableColor::Rgb {
                    r: 222,
                    g: 184,
                    b: 135,
                },
                command_prefix: SerializableColor::Named("cyan".to_string()),
                command_name: SerializableColor::Named("yellow".to_string()),
            },
            styles: ThemeStyles {
                border_type: BorderType::Rounded,
                bold_borders: false,
                spinners: super::types::SpinnerConfig {
                    speed_ms: 150,
                    running: super::types::SpinnerFrames {
                        frames: vec![
                            "▖".to_string(),
                            "▘".to_string(),
                            "▝".to_string(),
                            "▗".to_string(),
                        ],
                        colors: vec![],
                    },
                    completed: "●".to_string(),
                    failed: "○".to_string(),
                },
            },
            override_terminal_background: false,
        }
    }

    /// Get a built-in theme by name
    pub fn builtin(name: &str) -> Option<Self> {
        match name {
            "default" => Some(Self::default()),
            "lcars" => Some(Self::lcars()),
            "cyberpunk" => Some(Self::cyberpunk()),
            "solarized-dark" => Some(Self::solarized_dark()),
            "solarized-light" => Some(Self::solarized_light()),
            "matrix" => Some(Self::matrix()),
            "dracula" => Some(Self::dracula()),
            "gruvbox" => Some(Self::gruvbox()),
            "gruvbox-light" => Some(Self::gruvbox_light()),
            "github-light" => Some(Self::github_light()),
            "catppuccin-latte" => Some(Self::catppuccin_latte()),
            "nord-light" => Some(Self::nord_light()),
            "one-light" => Some(Self::one_light()),
            "paper" => Some(Self::paper()),
            "sepia" => Some(Self::sepia()),
            _ => None,
        }
    }

    /// List all built-in theme names
    pub fn builtin_names() -> Vec<&'static str> {
        vec![
            "default",
            "lcars",
            "cyberpunk",
            "solarized-dark",
            "solarized-light",
            "matrix",
            "dracula",
            "gruvbox",
            "gruvbox-light",
            "github-light",
            "catppuccin-latte",
            "nord-light",
            "one-light",
            "paper",
            "sepia",
        ]
    }
}
