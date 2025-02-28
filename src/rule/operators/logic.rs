use serde_json::Value;
use crate::rule::ArgType;
use super::{Rule, ValueCoercion};
use crate::Error;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub enum LogicType { And, Or, Not, DoubleBang }

pub struct LogicOperator;

impl LogicOperator {
    pub fn apply<'a>(&self, args: &'a ArgType, context: &'a Value, root: &'a Value, path: &str, logic_type: &LogicType) -> Result<Cow<'a, Value>, Error> {
        if let ArgType::Multiple(arg_arr) = args {
            match logic_type {
                LogicType::And => self.apply_and(arg_arr, context, root, path),
                LogicType::Or => self.apply_or(arg_arr, context, root, path),
                LogicType::Not => self.apply_not(arg_arr, context, root, path),
                LogicType::DoubleBang => self.apply_double_bang(arg_arr, context, root, path),
            }
        } else if let ArgType::Unary(arg) = args {
            match logic_type {
                LogicType::Not => self.apply_not(std::slice::from_ref(arg), context, root, path),
                LogicType::DoubleBang => self.apply_double_bang(std::slice::from_ref(arg), context, root, path),
                _ => Err(Error::Custom("Invalid Arguments".into()))
            }
        } else {
            Err(Error::Custom("Invalid Arguments".into()))
        }
    }

    fn apply_and<'a>(&self, args: &'a [Rule], context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        match args.len() {
            0 => Ok(Cow::Owned(Value::Bool(false))),
            1 => args[0].apply(context, root, path),
            _ => {
                for arg in &args[..args.len()-1] {
                    let value = arg.apply(context, root, path)?;
                    if !value.coerce_to_bool() {
                        return Ok(value);
                    }
                }
                args.last().unwrap().apply(context, root, path)
            }
        }
    }

    fn apply_or<'a>(&self, args: &'a [Rule], context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        match args.len() {
            0 => Ok(Cow::Owned(Value::Bool(false))),
            1 => args[0].apply(context, root, path),
            _ => {
                for arg in &args[..args.len()-1] {
                    let value = arg.apply(context, root, path)?;
                    if value.coerce_to_bool() {
                        return Ok(value);
                    }
                }
                args.last().unwrap().apply(context, root, path)
            }
        }
    }

    fn apply_not<'a>(&self, args: &[Rule], context: &Value, root: &Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        match args.len() {
            0 => Ok(Cow::Owned(Value::Bool(true))),
            _ => {
                let value = args[0].apply(context, root, path)?;
                Ok(Cow::Owned(Value::Bool(!value.coerce_to_bool())))
            }
        }
    }

    fn apply_double_bang<'a>(&self, args: &[Rule], context: &Value, root: &Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        match args.len() {
            0 => Ok(Cow::Owned(Value::Bool(false))),
            _ => {
                let value = args[0].apply(context, root, path)?;
                Ok(Cow::Owned(Value::Bool(value.coerce_to_bool())))
            }
        }
    }
}