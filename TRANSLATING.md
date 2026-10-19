# Guidelines for Translators

Thank you for helping translate Cfait :-)

When translating the application on Codeberg Translate / Weblate, please keep the following technical details in mind regarding "smart syntax" and search keywords (keys starting with `parser_` and `search_`):

## 1. Smart Syntax (`parser_*`)
These strings are used by the application to quickly add task metadata using the keyboard when creating tasks.
- **Multiple Options:** You can provide multiple synonyms separated by commas (e.g., `url:,lien:`). The parser will accept any of them.
- **No Spaces:** Do not put spaces after the commas (`a,b,c` is correct, `a, b, c` is wrong).
- **Keep Symbols First:** The app uses the *very first item* in the list to build the examples in the Help menu. Always keep symbols like `~`, `@`, or `@@` at the beginning of the list (e.g., `~,est:,dur:`).
- **Keep it Short:** Smart syntax is designed for fast typing. Stick to at most 2 real translations per keyword, and at most 6 entries in total once you add spelling variations (singular/plural, umlaut spellings like `außer,ausser`). Abbreviated forms for things like days or months (`mon,monday,mondays`) are exempt from the 2-translation limit. Accents are taken care of automatically: typing `sabado` also matches a token written `sábado`, so never list the unaccented spelling yourself.

## 2. Search Keywords (`search_is_*`)
These are the keywords users type in the search bar (like `is:done` or `is:ready`).
- You can translate the `is:` part to your language (e.g., `es:hecho`), or keep the English `is:` prefix if it feels more natural for technical users (e.g., `is:hecho`).
- You can provide multiple options separated by commas just like smart syntax (e.g., `is:nota,es:nota`).
- **Keep it Short:** The same limits as smart syntax apply: at most 2 real translations, at most 6 entries in total with spelling variations, and no need to list unaccented spellings (accents are handled automatically).

## 3. Placeholders
Keep placeholders like `%{count}` or `%{error}` exactly as they are. They will be replaced by the application at runtime.
