from ._lzss import install as _install_lzss

# Before anything imports pymobiledevice3: pure-Python lzss/lzfse instead of
# unsigned .pyd files that Smart App Control blocks (GitHub issue #2).
_install_lzss()
