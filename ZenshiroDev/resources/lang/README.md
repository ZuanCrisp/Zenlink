# ZenLink languages

ZenLink is written in English. Each other language is one gettext `.po` file in this
folder: `id.po` is Bahasa Indonesia. Any `.po` editor works (Poedit, Weblate,
Lokalize, or a plain text editor).

## Add a language

1. Copy `zenlink.pot` to `<code>.po`, where `<code>` is the language code
   (for example `ms.po` for Malay, `ja.po` for Japanese).
2. Put the language's own name in the header:
   `"Language-Name: Bahasa Melayu\n"`. Settings shows this name.
3. Fill in each `msgstr` with the translation of its `msgid`.
   - Keep `{placeholders}` such as `{device}` or `{n}` exactly as they are;
     ZenLink fills them in. A test checks that `id.po` keeps them.
   - An empty `msgstr`, or an entry marked `#, fuzzy`, shows the English text.
4. Try it without building ZenLink: put the file in
   `%LOCALAPPDATA%\ZenLink-Next\lang` (works for Setup, Portable and source, and
   survives updates) or in the `lang` folder next to `ZenLink.exe` (for Setup:
   `C:\Program Files\ZenLink\lang`, which also holds `zenlink.pot`). Restart ZenLink
   and pick the language in Settings. A file there also overrides the built-in
   `id.po`, so you can test fixes the same way.
5. Send the file as a pull request to this folder. To build it into ZenLink, add
   it to `BUILT_IN` in `apps/desktop/src/i18n.rs`.

## After changing texts in the code

Run `python ZenshiroDev/tooling/dev/extract_texts.py`. It rewrites `zenlink.pot`
and updates every `.po` file here: new texts are added untranslated, removed
ones are dropped, translations stay.

Texts built while ZenLink runs (backend messages, folder names) are not in the
code as `I18n.t(...)` or `i18n::tr(...)`. They are marked `#. added by hand` in
`id.po`; the script keeps them. Add new ones the same way.

## Right-to-left languages

Arabic, Hebrew, Persian and other right-to-left languages are welcome. Their text
shows, but the layout stays left-to-right (buttons and icons are not mirrored yet).
If you translate one, open an issue on GitHub so full right-to-left support can be
built and tested with you.
