project = "cask"
copyright = "2026, cyrenei"
author = "cyrenei"
release = "0.1.0"

extensions = [
    "sphinx_copybutton",
]

templates_path = ["_templates"]
exclude_patterns = ["_build"]

html_theme = "furo"
html_title = "cask"
html_theme_options = {
    "light_css_variables": {
        "color-brand-primary": "#2563eb",
        "color-brand-content": "#2563eb",
    },
    "dark_css_variables": {
        "color-brand-primary": "#60a5fa",
        "color-brand-content": "#60a5fa",
    },
}

html_static_path = ["_static"]

copybutton_prompt_text = r"^\$ "
copybutton_prompt_is_regexp = True
