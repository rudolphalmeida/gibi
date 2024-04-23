"""
Dependencies: pip install pydantic mypy ruff
"""

import argparse
import pathlib

from pydantic import BaseModel, Field


class Operand(BaseModel):
    name: str
    immediate: bool
    bytes: int | None = Field(default=None)


class Opcode(BaseModel):
    mnemonic: str
    bytes: int
    cycles: list[int]
    operands: list[Operand]
    immediate: bool
    flags: dict[str, str]


class Opcodes(BaseModel):
    unprefixed: dict[str, Opcode]
    cbprefixed: dict[str, Opcode]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "opcodes_file", type=pathlib.Path, help="Path to opcodes.json file"
    )

    args = parser.parse_args()
    if not args.opcodes_file.exists():
        print(f"{args.opcodes_file} does not exist")
        return

    data = open(args.opcodes_file).read()
    opcodes = Opcodes.model_validate_json(data)
    generate_format_strings(opcodes)


def generate_format_strings(opcodes: Opcodes) -> None:
    print("fn format_opcode(opcode: u8, arg1: u8, arg2: u8) -> String {")
    print("    match opcode {")
    for op in range(0x00, 0x100):
        key = f"0x{op:02X}"
        template = generate_format_opcode(opcodes.unprefixed[key])
        print(f'        {op} => format!("{template}"),')
    print("    }")
    print("}")


def generate_format_opcode(opcode: Opcode) -> str:
    operand_formats = []
    for operand in opcode.operands:
        operand_format = ""
        match operand.name:
            case (
                "A"
                | "B"
                | "C"
                | "D"
                | "E"
                | "H"
                | "L"
                | "BC"
                | "DE"
                | "HL"
                | "SP"
                | "AF"
            ) if operand.immediate is True:
                operand_format = f"{operand.name}"
            case "BC" | "DE" | "HL" if operand.immediate is False:
                operand_format = f"({operand.name})"
            case "d16":
                operand_format = "0x{arg2:02X}{arg1:02X}"
            case "a16":
                operand_format = "(0x{arg2:02X}{arg1:02X})"
            case "d8":
                operand_format = "0x{arg1:02X}"
            case "r8":
                operand_format = "0x{arg1:02X}"
            case "a8":
                operand_format = "(0xFF{arg1:02X})"
            case "NZ" | "Z" | "NC" | "C":
                operand_format = f"{operand.name}"
            case name:
                operand_format = f"0x{name[:2]}"
        operand_formats.append(operand_format)

    return f"{opcode.mnemonic} {', '.join(operand_formats)}"


if __name__ == "__main__":
    main()
