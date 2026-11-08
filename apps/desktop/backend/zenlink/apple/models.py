"""Device model names from the chip ID in the UDID (no USB I/O).

A modern UDID starts with the SoC's chip ID (CPID), e.g. 00008120-… is A16.
Together with the product family from the USB descriptor (iPhone/iPad) that
names most devices; where Apple used one chip for several models, the series
is shown instead. Names the user sets in the app take precedence.
"""

from __future__ import annotations

import json
import threading

from zenlink.paths import DATA_DIR

_IPHONE = {
    "8020": "iPhone XS / XR",
    "8030": "iPhone 11 series",
    "8101": "iPhone 12 series",
    "8110": "iPhone 13 / 14",
    "8120": "iPhone 15 / 14 Pro",
    "8130": "iPhone 15 Pro",
    "8140": "iPhone 16 series",
    "8150": "iPhone 17 series",
}

_IPAD = {
    "8020": "iPad (8th gen)",
    "8030": "iPad (9th gen)",
    "8101": "iPad (10th gen)",
    "8103": "iPad Pro / Air (M1)",
    "8110": "iPad mini (6th gen)",
    "8112": "iPad Pro / Air (M2)",
    "8120": "iPad (11th gen)",
    "8122": "iPad Air (M3)",
    "8130": "iPad mini (A17 Pro)",
    "8132": "iPad Pro (M4)",
}

# ProductType (from lockdown) -> marketing name.
PRODUCT_TYPES = {
    "iPhone14,4": "iPhone 13 mini", "iPhone14,5": "iPhone 13", "iPhone14,2": "iPhone 13 Pro",
    "iPhone14,3": "iPhone 13 Pro Max", "iPhone14,6": "iPhone SE (3rd gen)", "iPhone14,7": "iPhone 14",
    "iPhone14,8": "iPhone 14 Plus", "iPhone15,2": "iPhone 14 Pro", "iPhone15,3": "iPhone 14 Pro Max",
    "iPhone15,4": "iPhone 15", "iPhone15,5": "iPhone 15 Plus", "iPhone16,1": "iPhone 15 Pro",
    "iPhone16,2": "iPhone 15 Pro Max", "iPhone17,3": "iPhone 16", "iPhone17,4": "iPhone 16 Plus",
    "iPhone17,1": "iPhone 16 Pro", "iPhone17,2": "iPhone 16 Pro Max", "iPhone17,5": "iPhone 16e",
    "iPhone18,3": "iPhone 17", "iPhone18,4": "iPhone Air", "iPhone18,1": "iPhone 17 Pro",
    "iPhone18,2": "iPhone 17 Pro Max",
    "iPad13,18": "iPad (10th gen)", "iPad13,19": "iPad (10th gen)",
    "iPad15,7": "iPad (11th gen)", "iPad15,8": "iPad (11th gen)",
    "iPad14,1": "iPad mini (6th gen)", "iPad14,2": "iPad mini (6th gen)",
    "iPad16,1": "iPad mini (A17 Pro)", "iPad16,2": "iPad mini (A17 Pro)",
    "iPad14,8": "iPad Air 11-inch (M2)", "iPad14,9": "iPad Air 11-inch (M2)",
    "iPad14,10": "iPad Air 13-inch (M2)", "iPad14,11": "iPad Air 13-inch (M2)",
    "iPad15,3": "iPad Air 11-inch (M3)", "iPad15,4": "iPad Air 11-inch (M3)",
    "iPad15,5": "iPad Air 13-inch (M3)", "iPad15,6": "iPad Air 13-inch (M3)",
    "iPad16,3": "iPad Pro 11-inch (M4)", "iPad16,4": "iPad Pro 11-inch (M4)",
    "iPad16,5": "iPad Pro 13-inch (M4)", "iPad16,6": "iPad Pro 13-inch (M4)",
}

_NAMES_FILE = DATA_DIR / "devices.json"
_lock = threading.Lock()


def _custom_names() -> dict:
    try:
        return json.loads(_NAMES_FILE.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def _save(names: dict) -> None:
    _NAMES_FILE.write_text(json.dumps(names, indent=2), encoding="utf-8")


def set_custom_name(device_id: str, name: str) -> None:
    with _lock:
        names = _custom_names()
        name = name.strip()[:40]
        if name:
            names[device_id] = name
        else:
            names.pop(device_id, None)
        _save(names)


def remember_product(device_id: str, product_type: str) -> None:
    """Store the exact model learned from lockdown."""
    model = PRODUCT_TYPES.get(product_type)
    if not model:
        return
    with _lock:
        names = _custom_names()
        known = names.get("_models", {})
        if known.get(device_id) != model:
            known[device_id] = model
            names["_models"] = known
            _save(names)


def model_name(device_id: str, family: str) -> str:
    """family is 'iPhone', 'iPad' or 'iOS device'."""
    names = _custom_names()
    custom = names.get(device_id)
    if isinstance(custom, str) and custom:
        return custom
    learned = names.get("_models", {}).get(device_id)
    if learned:
        return learned
    cpid = device_id[4:8].upper() if len(device_id) >= 8 and device_id.startswith("0000") else ""
    table = _IPAD if family == "iPad" else _IPHONE if family == "iPhone" else {}
    return table.get(cpid, family)
