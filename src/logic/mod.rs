//! Logic expression representation and evaluation.
//!
//! This module provides types and functions for representing and evaluating
//! logic expressions using arena allocation for improved performance.

mod ast;
mod token;
mod parser;
mod evaluator;
pub mod error;
mod operators;
mod optimizer;
mod jsonlogic;

pub use ast::Logic;
pub use token::{Token, OperatorType};
pub use parser::{parse_json, parse_str};
pub use evaluator::evaluate;
pub use error::{LogicError, Result};
pub use jsonlogic::JsonLogic;

// Re-export operator types
pub use operators::comparison::ComparisonOp;
pub use operators::arithmetic::ArithmeticOp;
pub use operators::control::ControlOp;
pub use operators::array::ArrayOp;
pub use operators::string::StringOp;

/// Trait for types that can be converted into a Logic expression.
pub trait IntoLogic {
    /// Converts the value into a Logic expression, allocating in the given arena.
    fn to_logic<'a>(&self, arena: &'a crate::arena::DataArena) -> Result<Logic<'a>>;
}

// Implement IntoLogic for common types
impl IntoLogic for serde_json::Value {
    fn to_logic<'a>(&self, arena: &'a crate::arena::DataArena) -> Result<Logic<'a>> {
        let token = parse_json(self, arena)?;
        
        // Apply static optimization
        let optimized_token = optimizer::optimize(token, arena)?;
        
        Ok(Logic::new(optimized_token, arena))
    }
}

impl IntoLogic for &str {
    fn to_logic<'a>(&self, arena: &'a crate::arena::DataArena) -> Result<Logic<'a>> {
        let token = parse_str(self, arena)?;
        
        // Apply static optimization
        let optimized_token = optimizer::optimize(token, arena)?;
        
        Ok(Logic::new(optimized_token, arena))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;
    use crate::value::{DataValue, FromJson};
    use serde_json::json;

    #[test]
    fn test_simple_logic() {
        let arena = DataArena::new();
        
        // Create a simple comparison logic
        let json_logic = json!({"==": [{"var": "a"}, 10]});
        let logic = json_logic.to_logic(&arena).unwrap();
        
        // Create test data
        let data_json = json!({"a": 10});
        let data = DataValue::from_json(&data_json, &arena);
        
        // Evaluate logic
        let result = evaluate(logic.root(), &data, &arena).unwrap();
        
        // Verify result
        assert_eq!(result.as_bool(), Some(true));
    }
    
    #[test]
    fn test_optimized_logic() {
        let arena = DataArena::new();
        
        // Create a logic with static parts that can be optimized
        let json_logic = json!({"and": [
            {"==": [{"var": "a"}, 10]},
            {"==": [{"+":[1, 2]}, 3]}
        ]});
        let logic = json_logic.to_logic(&arena).unwrap();
        
        // The second part of the AND should be optimized to a literal true
        let (op_type, args) = logic.root().as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Control(ControlOp::And));
        
        // Check that args is an ArrayLiteral
        assert!(args.is_array_literal());
        let array_tokens = args.as_array_literal().unwrap();
        assert_eq!(array_tokens.len(), 2);
        
        // First argument should still be a comparison
        assert!(array_tokens[0].is_operator());

        // Second argument should be optimized to a literal
        assert!(array_tokens[1].is_literal());
        assert_eq!(array_tokens[1].as_literal().unwrap().as_bool(), Some(true));
        
        // Create test data
        let data_json = json!({"a": 10});
        let data = DataValue::from_json(&data_json, &arena);
        
        // Evaluate logic
        let result = evaluate(logic.root(), &data, &arena).unwrap();
        
        // Verify result
        assert_eq!(result.as_bool(), Some(true));
    }
} 