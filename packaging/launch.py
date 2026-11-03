"""Entry point for the standalone (PyInstaller) build."""

import multiprocessing

from zenlink.__main__ import main

if __name__ == "__main__":
    multiprocessing.freeze_support()
    main()
