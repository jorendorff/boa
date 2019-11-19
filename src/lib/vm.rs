///! VM for compiling JavaScript to instructions and executing the instructions.
use crate::{
    builtins::value::{ResultValue, Value, ValueData},
    environment::lexical_environment::VariableScope,
    realm::Realm,
    syntax::ast::{
        constant::Const,
        expr::{Expr, ExprDef},
        op::{BinOp, NumOp},
    },
};

//
// =====================================================================
// Part 1: The "bytecode" language of the VM
//
// The VM executes a sequence of instructions. Each instruction is pretty
// simple and doesn't include arbitrarily complex subexpressions or statements.
//
// Part 2 (below) is the VM itself, which reads and executes these instructions.
//
// Part 3 is the compiler that transforms a JS AST into an instruction sequence.
//

/// The index of a register.
///
/// The VM consists of an array of 256 values called "registers" that are used
/// as temporary storage for JS values we're working on. All instructions refer
/// to at least one register.

#[derive(Copy, Clone, Debug)]
pub struct Register(u8);

/// A variable name.
#[derive(Debug)]
pub struct Identifier(String);

/// Each instruction does something to some register, or else to a variable or a property.
#[derive(Debug)]
pub enum Instruction {
    /// Get the value of a variable (`name`) and store it in the `target` register.
    GetName { target: Register, name: Identifier },

    /// Store the value of the `source` register in the variable `name`.
    SetName { name: Identifier, source: Register },

    /// Add two values in registers (`left` and `right`). Store the result in
    /// the `target` register.
    Add {
        target: Register,
        left: Register,
        right: Register,
    },

    /// Replace the value of the `target` register with the given integer `value`.
    IntLiteral { target: Register, value: i32 },
}

//
// =====================================================================
// Part 2: The VM itself
//

#[derive(Debug)]
pub struct VM {
    realm: Realm,
    registers: Vec<ValueData>,
}

impl VM {
    pub fn new(realm: Realm) -> VM {
        VM {
            realm,
            registers: vec![ValueData::Undefined; 256],
        }
    }

    fn get(&self, register: Register) -> ValueData {
        self.registers[register.0 as usize].clone()
    }

    fn set(&mut self, target: Register, value: ValueData) {
        self.registers[target.0 as usize] = value;
    }

    pub fn run(&mut self, instructions: &[Instruction]) -> ResultValue {
        let mut index = 0;

        while index < instructions.len() {
            match &instructions[index] {
                Instruction::GetName { target, name } => {
                    self.set(
                        *target,
                        (*self.realm.environment.get_binding_value(&name.0)).clone(),
                    );
                }

                Instruction::SetName { name, source } => {
                    let val = Value::new(self.get(*source));
                    if self.realm.environment.has_binding(&name.0) {
                        // Binding already exists
                        self.realm
                            .environment
                            .set_mutable_binding(&name.0, val, true);
                    } else {
                        self.realm.environment.create_mutable_binding(
                            name.0.clone(),
                            true,
                            VariableScope::Function,
                        );
                        self.realm.environment.initialize_binding(&name.0, val);
                    }
                }

                Instruction::Add {
                    target,
                    left,
                    right,
                } => {
                    self.set(*target, self.get(*left) + self.get(*right));
                }

                Instruction::IntLiteral { target, value } => {
                    self.set(*target, ValueData::Integer(*value));
                }
            }
            index += 1;
        }

        // The result of evaluating the script is whatever's left in register 0.
        Ok(Value::new(self.get(Register(0))))
    }
}

// =====================================================================
// Part 3: The compiler

pub fn compile_expr(target: Register, expr: &Expr, out: &mut Vec<Instruction>) {
    match &expr.def {
        ExprDef::Const(Const::Int(value)) => {
            out.push(Instruction::IntLiteral {
                target,
                value: *value,
            });
        }

        ExprDef::Const(Const::Num(value)) => {
            let f: f64 = *value;
            if f.is_nan()
                || f > i32::max_value() as f64
                || f < i32::min_value() as f64
                || f.fract() != 0.0
            {
                unimplemented!("numeric constant that doesn't fit in i32 range");
            }
            out.push(Instruction::IntLiteral {
                target,
                value: f as i32,
            });
        }

        ExprDef::Local(name) => {
            out.push(Instruction::GetName {
                target,
                name: Identifier(name.clone()),
            });
        }

        ExprDef::Assign(target_expr, value_expr) => match &target_expr.def {
            ExprDef::Local(name) => {
                compile_expr(target, value_expr, out);
                out.push(Instruction::SetName {
                    name: Identifier(name.clone()),
                    source: target,
                });
            }

            _ => unimplemented!("assignment to something other than a variable"),
        },

        ExprDef::BinOp(BinOp::Num(NumOp::Add), left, right) => {
            // First push instructions to compute the left expression.
            compile_expr(target, left, out);

            // Then the right expression, making sure to put the result in a
            // different register.
            if target.0 == 255 {
                panic!("ran out of registers :(");
            }
            let tmp_register = Register(target.0 + 1);
            compile_expr(tmp_register, right, out);

            // Finally, push the instruction that actually adds the two
            // together.
            out.push(Instruction::Add {
                target,
                left: target,
                right: tmp_register,
            });
        }

        ExprDef::Block(exprs) => {
            for expr in exprs {
                compile_expr(target, expr, out);
            }
        }

        _ => unimplemented!("{:?}", expr.def),
    }
}

pub fn compile(expr: &Expr) -> Vec<Instruction> {
    let mut instructions = vec![];
    compile_expr(Register(0), expr, &mut instructions);
    instructions
}

// =====================================================================
// Part 4: Tests!

#[cfg(test)]
mod tests {
    use crate::{
        parser_expr,
        realm::Realm,
        vm::{self, VM},
    };

    /// Create a clean VM and execute the code
    pub fn exec(src: &str) -> String {
        let expr = parser_expr(src);
        let instructions = vm::compile(&expr);

        let realm = Realm::create();
        let mut engine = VM::new(realm);
        let result = engine.run(&instructions);

        match result {
            Ok(v) => v.to_string(),
            Err(v) => format!("{}: {}", "Error", v.to_string()),
        }
    }

    #[test]
    fn test_compilation() {
        assert_eq!(exec("2 + 2"), "4");
    }
}
