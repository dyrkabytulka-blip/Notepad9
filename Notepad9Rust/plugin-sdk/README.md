# Плагины Notepad9

Плагин — это обычная DLL (`plugin.dll`), написанная на Rust (`crate-type = ["cdylib"]`).
Notepad9 загружает её при запуске. Плагин может:

- добавлять пункты в меню **Plugins** (с горячими клавишами) и кнопки на панель инструментов;
- открывать свои **окна**: плавающие, панели справа или панели снизу, с кнопками, полями ввода,
  галочками, ползунками, списками, прогресс-барами;
- **читать и менять текст**: весь текст, выделение, строки, позицию курсора;
- работать с **вкладками и файлами**: список вкладок, переключение, открытие, сохранение, новые вкладки;
- получать **события**: файл открыт или сохранён, текст изменён, вкладка переключена, введён символ,
  таймер;
- запускать встроенные команды редактора (`find`, `wrap`, `zoom_in`…), команды в консоли, работать с
  буфером обмена;
- хранить свои **настройки** (они сохраняются между запусками).

## Куда класть плагины

```
%APPDATA%\Notepad9\plugins\my_plugin.dll                ← для текущего пользователя
<папка Notepad9>\plugins\my_plugin.dll                  ← для всех (так ставит установщик)
...\plugins\my_plugin\my_plugin.dll                     ← или в своей подпапке (рядом можно держать файлы плагина)
```

После этого перезапустите Notepad9. Список плагинов, их ошибки и журнал (log) — в
**Plugins → Plugin Manager…**. Там же плагин можно выключить (вступает в силу после перезапуска).

## Быстрый старт

```
cargo new --lib hello_plugin
```

`Cargo.toml`:

```toml
[package]
name = "hello_plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
notepad9_plugin = { path = "путь/к/Notepad9Rust/plugin-sdk" }
```

`src/lib.rs`:

```rust
use notepad9_plugin::*;

const CMD_HELLO: u32 = 1;
const CMD_PANEL: u32 = 2;
const WIN: u32 = 1;

#[derive(Default)]
struct Hello {
    clicks: u32,
}

impl Plugin for Hello {
    fn info() -> Info {
        Info { name: "Hello", version: "0.1.0", author: "Я", description: "Пример плагина" }
    }

    // регистрация: меню, кнопки, окна
    fn init(&mut self, host: &Host) {
        host.add_menu_item("Hello/Сказать привет", "Ctrl+Alt+H", CMD_HELLO);
        host.add_menu_item("Hello/Панель", "", CMD_PANEL);
        host.add_toolbar_button("star", "Hello", CMD_PANEL);
    }

    fn on_command(&mut self, host: &Host, command: u32) {
        match command {
            CMD_HELLO => host.message("Hello", &format!("В файле {} строк", host.line_count())),
            CMD_PANEL => host.toggle_window(WIN, "Hello", WindowKind::RightPanel, 250.0, 0.0),
            _ => {}
        }
    }

    // содержимое окна: вызывается каждый кадр, пока окно открыто
    fn ui(&mut self, host: &Host, _window: u32, ui: &mut Ui) {
        ui.heading("Привет!");
        ui.label(&format!("Курсор на строке {}", host.line_of(host.caret()) + 1));
        let name = ui.text_input("name", "Ваше имя");
        if ui.button("insert", "Вставить в текст") {
            host.replace_selection(&format!("Привет, {name}!"));
            self.clicks += 1;
        }
        ui.weak(&format!("Нажато: {}", self.clicks));
    }
}

export_plugin!(Hello);
```

Сборка:

```
cargo build --release
copy target\release\hello_plugin.dll %APPDATA%\Notepad9\plugins\
```

## Как устроен плагин

- Ваша структура реализует трейт `Plugin` (и `Default`); макрос `export_plugin!(Тип)` экспортирует из DLL
  функцию `n9_plugin_entry`.
- `init` вызывается один раз при загрузке. В нём регистрируются пункты меню, кнопки, открываются окна.
- `on_command(command)` — нажат пункт меню, кнопка панели или горячая клавиша с этим номером команды.
- `on_event(&Event)` — что-то случилось в редакторе (см. ниже).
- `ui(window, &mut Ui)` — описывает содержимое окна `window`. Вызывается каждый кадр, пока окно открыто.
- `shutdown` — Notepad9 закрывается (удобно сохранить настройки).

Все функции `Host` вызываются **только из этих методов** (из UI-потока). Единственное исключение —
`host.request_repaint()`: её можно вызывать из своего потока, чтобы окно перерисовалось.

Паника внутри плагина не роняет редактор: SDK перехватывает её, пишет в журнал плагина и показывает
сообщение в строке состояния. Но ошибки доступа к памяти (`unsafe`) уронят и Notepad9 — плагин работает
в том же процессе.

## Окна

```rust
host.open_window(id, "Заголовок", WindowKind::Floating, 400.0, 300.0);   // плавающее окно
host.open_window(id, "Заголовок", WindowKind::RightPanel, 260.0, 0.0);   // панель справа (ширина)
host.open_window(id, "Заголовок", WindowKind::BottomPanel, 0.0, 180.0);  // панель снизу (высота)
host.close_window(id);
host.toggle_window(id, ...);      // открыть / закрыть
host.is_window_open(id)           // пользователь мог закрыть окно сам
```

Окно описывается в `ui()` в стиле immediate mode: каждый кадр вы заново перечисляете элементы.
Значения полей ввода хранит редактор (по `id` элемента), поэтому `text_input`, `checkbox`, `slider`
сами помнят, что ввёл пользователь.

| Метод `Ui` | Что делает |
|---|---|
| `label`, `heading`, `strong`, `weak`, `small`, `monospace`, `colored(text, (r,g,b))` | текст |
| `button(id, text) -> bool` | кнопка; `true` один раз после нажатия |
| `checkbox(id, text, initial) -> bool`, `set_checkbox(id, v)` | галочка |
| `text_input(id, hint) -> String`, `text_area(id, hint, rows)`, `set_input(id, text)` | поле ввода |
| `slider(id, text, min, max, initial) -> f64`, `set_slider(id, v)` | ползунок |
| `selectable(id, text, selected) -> bool` | строка списка; `true` после клика |
| `link(id, text) -> bool` | ссылка |
| `progress(0.0..1.0, text)`, `separator()`, `space(px)` | прочее |
| `horizontal(|ui| ..)`, `wrap(|ui| ..)`, `vertical(|ui| ..)` | раскладка |
| `scroll(id, max_height, |ui| ..)`, `collapsing(id, title, open, |ui| ..)`, `group(|ui| ..)` | блоки |

Нажатие кнопки приходит в следующем кадре после клика — это незаметно для пользователя.

## Текст и вкладки

Позиции — это **номера символов** (не байтов), строки нумеруются с 0.

```rust
host.text()                    // весь текст активной вкладки
host.set_text(s)               // заменить весь текст (одним шагом отмены)
host.selection() -> (a, b)     // выделение, a <= b
host.set_selection(a, b)
host.selected_text() / host.replace_selection(s)
host.replace_range(a, b, s) / host.insert_text(at, s)
host.caret() / host.text_len() / host.line_count()
host.line_text(n) / host.line_of(pos) / host.line_start(n) / host.goto_line(n)
host.language()                // "Rust", "Python", "Markdown"… или "" для простого текста

host.tab_count() / host.active_tab() / host.activate_tab(i)
host.tab_title(i) / host.tab_path(i) / host.tab_modified(i)
host.new_tab(text) / host.open_file(path) / host.save()
```

Переводы строк во вставляемом тексте приводятся к окончаниям строк файла (CR+LF, LF или CR).

## События

| `Event` | Когда |
|---|---|
| `Startup` | все плагины загружены |
| `TabActivated { tab }` | переключена вкладка |
| `FileOpened { tab, path }` | файл открыт (или перезагружен) |
| `FileSaved { tab, path }` | файл сохранён |
| `TextChanged { tab }` | текст изменился (не чаще раза за кадр) |
| `SelectionChanged { tab }` | сдвинулся курсор или выделение |
| `CharTyped { tab, ch }` | пользователь ввёл символ |
| `Timer` | сработал таймер `host.set_timer(ms)` (`0` — выключить) |

Совет: в `on_event` просто запомните «надо пересчитать», а считайте в `ui()` — тогда работа делается,
только когда окно плагина видно.

## Прочее

```rust
host.run_command("find");            // встроенные команды: new, open, save, save_as, save_all, close_tab,
                                     // reload, undo, redo, cut, copy, paste, select_all, find, replace,
                                     // find_next, find_prev, find_files, goto, wrap, toggle_lnum, zoom_in,
                                     // zoom_out, zoom_reset, console, run, upper, lower, sort_asc, sort_desc,
                                     // uniq, dup_line, del_line, md_preview, customize, pin, about ...
                                     // а также "open:<путь>", "scheme:<синтаксис>"
host.console_run("cargo build", "") // команда во встроенной консоли ("" = папка текущего файла)
host.set_clipboard(s) / host.clipboard()
host.show_info(s)                    // короткое сообщение в строке состояния
host.message(title, text)            // окно с кнопкой OK
host.log(s)                          // журнал плагина (виден в Plugin Manager)
host.setting(key) / host.set_setting(key, value)   // %APPDATA%\Notepad9\plugin-data\<имя dll>.json
host.app_version()
```

Встроенные значки для `add_toolbar_button`: `new`, `open`, `save`, `find`, `replace`, `find_files`,
`goto`, `mark`, `star`, `star_add`, `wrap`, `ruler`, `terminal`, `run`, `stop`, `clear`, `preview`,
`settings`, `font`, `about`, `pin`, `print`, `zoom_in`, `zoom_out`, `ac_word`, `ac_key`, `md_bold`,
`md_code`, `md_table` и другие из `tools/make_icons.py`. Свой значок — PNG 16×16 или 32×32:

```rust
static ICON: &[u8] = include_bytes!("../icon.png");
host.add_toolbar_button_png(ICON, "Подсказка", CMD);
```

Горячие клавиши: `"Ctrl+Alt+W"`, `"Ctrl+Shift+F9"`, `"F8"`. Обычная буква без Ctrl/Alt не принимается
(она нужна для ввода текста). Если сочетание уже занято самим Notepad9, сработает команда редактора.

## Примеры

В папке `Notepad9Rust/plugins`:

- **word_stats** — панель справа со статистикой (слова, символы, время чтения, частые слова);
  меню с горячей клавишей, кнопка на панели, события, настройки.
- **todo_list** — панель снизу со всеми TODO / FIXME / HACK / NOTE файла; фильтр, галочки, переход к
  строке по клику, вставка TODO-комментария.
- **text_tools** — плавающее окно и меню: Base64, URL, HTML, JSON, работа со строками, свой PNG-значок,
  параметры с ползунками, которые сохраняются.

Примеры — отдельный workspace: `cargo build --release` в папке `Notepad9Rust/plugins`, DLL появятся в
`plugins/target/release`. Установщик (`installer/build-installer-rust.cmd`) собирает и кладёт их сам.

Собирайте плагины с `panic = "unwind"` (это значение по умолчанию). С `panic = "abort"` паника в плагине
закроет весь Notepad9.

## Двоичный интерфейс

Связь редактора и плагина — чистый C ABI (`src/abi.rs`): функция
`n9_plugin_entry(host: *const HostApi, plugin: u32) -> *const PluginDesc`, таблица функций `HostApi`
и `UiApi`. Поэтому плагин, собранный любой версией Rust, работает с любой сборкой Notepad9 той же
версии API (`API_VERSION`). Новые функции добавляются только в конец структур. При желании плагин
можно написать и на C/C++/Zig — достаточно повторить эти структуры.

## Безопасность

Плагин — это машинный код с теми же правами, что и Notepad9. Ставьте только плагины, которым доверяете.
