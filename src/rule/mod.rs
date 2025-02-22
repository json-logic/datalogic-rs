mod operators;

use serde_json::Value;
use crate::Error;
use crate::JsonLogic;
use std::borrow::Cow;

pub use operators::*;

static VAL_OP: ValOperator = ValOperator;
static EXISTS_OP: ExistsOperator = ExistsOperator;
static VAR_OP: VarOperator = VarOperator;
static LOGIC_OP: LogicOperator = LogicOperator;
static COMPARE_OP: CompareOperator = CompareOperator;
static ARITHMETIC_OP: ArithmeticOperator = ArithmeticOperator;

static IF_OP: IfOperator = IfOperator;
static TERNARY_OP: TernaryOperator = TernaryOperator;
static COALESCE_OP: CoalesceOperator = CoalesceOperator;

static MAP_OP: MapOperator = MapOperator;
static FILTER_OP: FilterOperator = FilterOperator;
static REDUCE_OP: ReduceOperator = ReduceOperator;
static MERGE_OP: MergeOperator = MergeOperator;
static ARRAY_PREDICATE_OP: ArrayPredicateOperator = ArrayPredicateOperator;

static MISSING_OP: MissingOperator = MissingOperator;
static MISSING_SOME_OP: MissingSomeOperator = MissingSomeOperator;

static IN_OP: InOperator = InOperator;
static CAT_OP: CatOperator = CatOperator;
static SUBSTR_OP: SubstrOperator = SubstrOperator;

static PRESERVE_OP: PreserveOperator = PreserveOperator;
static TRY_OP: TryOperator = TryOperator;

#[derive(Debug, Clone)]
pub enum ArgType {
    Unary(Box<Rule>),
    Multiple(Vec<Rule>),
}

/// # Rule Construction
/// 
/// The `Rule` struct represents a parsed JSON Logic expression.
/// This struct encapsulates logic for evaluating expressions dynamically.
/// 
/// ## Example
/// ```rust
/// use datalogic_rs::Rule;
/// use serde_json::json;
/// 
/// let rule = Rule::from_value(&json!({"==": [1, 1]})).unwrap();
/// let result = rule.apply(&json!({})).unwrap();
/// ```
#[derive(Debug, Clone)]
pub enum Rule {
    Value(Value),
    Array(Vec<Rule>),

    Val(ArgType),
    Var(Box<Rule>, Option<Box<Rule>>),
    Compare(CompareType, Vec<Rule>),
    Arithmetic(ArithmeticType, ArgType),
    Logic(LogicType, ArgType),
    Exists(ArgType),
    
    // Control operators
    If(ArgType),
    Ternary(Vec<Rule>),
    Coalesce(Vec<Rule>),
    
    // String operators
    In(Box<Rule>, Box<Rule>),
    Cat(Vec<Rule>),
    Substr(Box<Rule>, Box<Rule>, Option<Box<Rule>>),
    
    // Array operators
    Map(Box<Rule>, Box<Rule>),
    Filter(Box<Rule>, Box<Rule>),
    Reduce(Box<Rule>, Box<Rule>, Box<Rule>),
    Merge(Vec<Rule>),
    ArrayPredicate(ArrayPredicateType, Box<Rule>, Box<Rule>),

    // Missing operators
    Missing(Vec<Rule>),
    MissingSome(Vec<Rule>),

    // Special operators
    Preserve(ArgType),
    Throw(Box<Rule>),
    Try(ArgType),

    Custom(String, Vec<Rule>),
}

impl Rule {
    fn is_static(&self) -> bool {
        match self {
            Rule::Value(_) => true,
            Rule::Missing(_) | Rule::MissingSome(_) => false,
            Rule::Var(_, _) => false,
            Rule::Val(_) => false,
            Rule::Exists(_) => false,

            Rule::Map(array_rule, mapper) => {
                array_rule.is_static() && mapper.is_static()
            }
            Rule::Reduce(array_rule, reducer, initial) => {
                array_rule.is_static() && reducer.is_static() && initial.is_static()
            }
            Rule::Filter(array_rule, predicate) => {
                array_rule.is_static() && predicate.is_static()
            }
            Rule::ArrayPredicate(_, array_rule, predicate) => {
                array_rule.is_static() && predicate.is_static()
            }

            Rule::In(search, target) => {
                search.is_static() && target.is_static()
            }
            Rule::Substr(string, start, length) => {
                if length.is_none() {
                    string.is_static() && start.is_static()
                } else {
                    string.is_static() && start.is_static() && length.as_deref().unwrap().is_static()
                }
            }

            Rule::Compare(_, args) => {
                args.iter().all(|r| r.is_static())
            }
            Rule::Logic(_, args) => {
                match args {
                    ArgType::Multiple(arr) => arr.iter().all(|r| r.is_static()),
                    ArgType::Unary(r) => r.is_static(),
                }
            }
            Rule::Arithmetic(_, args) => {
                match args {
                    ArgType::Multiple(arr) => arr.iter().all(|r| r.is_static()),
                    ArgType::Unary(r) => r.is_static(),
                }
            }
            Rule::Preserve(args) => {
                match args {
                    ArgType::Multiple(arr) => arr.iter().all(|r| r.is_static()),
                    ArgType::Unary(r) => r.is_static(),
                }
            }

            Rule::If(args) => {
                match args {
                    ArgType::Multiple(arr) => arr.iter().all(|r| r.is_static()),
                    ArgType::Unary(r) => r.is_static(),
                }
            }

            Rule::Array(args) |
            Rule::Ternary(args) |
            Rule::Coalesce(args) |
            Rule::Merge(args) |
            Rule::Cat(args) => args.iter().all(|r| r.is_static()),
            Rule::Throw(rule) => rule.is_static(),
            Rule::Try(args) => {
                match args {
                    ArgType::Multiple(arr) => arr.iter().all(|r| r.is_static()),
                    ArgType::Unary(r) => r.is_static(),
                }
            }
            Rule::Custom(_, args) => args.iter().all(|r| r.is_static()),
        }
    }

    fn optimize_args(args: &[Rule]) -> Result<Vec<Rule>, Error> {
        args.iter()
            .cloned()
            .map(Self::optimize_rule)
            .collect()
    }

    fn optimize_rule(rule: Rule) -> Result<Rule, Error> {
        match rule {
            // Never optimize these
            Rule::Missing(_) | Rule::MissingSome(_) => Ok(rule),
            Rule::Value(_) => Ok(rule),

            // Handle static evaluation
            rule if rule.is_static() => {
                rule.apply(&Value::Null)
                    .map(|v| Rule::Value(v.into_owned()))
                    .or(Ok(rule))
            },

            // Process arrays
            Rule::Array(args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Array(optimized))
            },
            Rule::Var(path, default) => {
                let optimized_path = Self::optimize_rule(*path)?;
                let optimized_default = default.map(|d| Self::optimize_rule(*d)).transpose()?;
                Ok(Rule::Var(Box::new(optimized_path), optimized_default.map(Box::new)))
            },
            Rule::Val(path) => {
                match path {
                    ArgType::Multiple(arr) => {
                        let optimized = Self::optimize_args(&arr)?;
                        Ok(Rule::Val(ArgType::Multiple(optimized)))
                    },
                    ArgType::Unary(rule) => {
                        let optimized = Self::optimize_rule(*rule)?;
                        Ok(Rule::Val(ArgType::Unary(Box::new(optimized))))
                    }
                }
            },
            Rule::Exists(path) => {
                match path {
                    ArgType::Multiple(arr) => {
                        let optimized = Self::optimize_args(&arr)?;
                        Ok(Rule::Exists(ArgType::Multiple(optimized)))
                    },
                    ArgType::Unary(rule) => {
                        let optimized = Self::optimize_rule(*rule)?;
                        Ok(Rule::Exists(ArgType::Unary(Box::new(optimized))))
                    }
                }
            },
            Rule::Arithmetic(op, args) => {
                match args {
                    ArgType::Multiple(arr) => {
                        let optimized = Self::optimize_args(&arr)?;
                        Ok(Rule::Arithmetic(op, ArgType::Multiple(optimized)))
                    },
                    ArgType::Unary(rule) => {
                        let optimized = Self::optimize_rule(*rule)?;
                        Ok(Rule::Arithmetic(op, ArgType::Unary(Box::new(optimized))))
                    }
                }
            },
            Rule::Map(array_rule, predicate) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::Map(Box::new(optimized_array_rule), Box::new(optimized_predicate)))
            },
            Rule::ArrayPredicate(op, array_rule, predicate) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::ArrayPredicate(op, Box::new(optimized_array_rule), Box::new(optimized_predicate)))
            },
            Rule::Filter(array_rule, predicate) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::Filter(Box::new(optimized_array_rule), Box::new(optimized_predicate)))
            },
            Rule::Reduce(array_rule, predicate, initial) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_initial = Self::optimize_rule(*initial)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::Reduce(Box::new(optimized_array_rule), Box::new(optimized_predicate), Box::new(optimized_initial)))
            },

            Rule::In(search, target) => {
                let optimized_search = Self::optimize_rule(*search)?;
                let optimized_target = Self::optimize_rule(*target)?;
                Ok(Rule::In(Box::new(optimized_search), Box::new(optimized_target)))
            },
            Rule::Substr(string, start, length) => {
                let optimized_string = Self::optimize_rule(*string)?;
                let optimized_start = Self::optimize_rule(*start)?;
                let optimized_length = length.map(|l| Self::optimize_rule(*l)).transpose()?;
                Ok(Rule::Substr(Box::new(optimized_string), Box::new(optimized_start), optimized_length.map(Box::new)))
            },
            Rule::Preserve(args) => {
                match args {
                    ArgType::Multiple(arr) => {
                        let optimized = Self::optimize_args(&arr)?;
                        Ok(Rule::Preserve(ArgType::Multiple(optimized)))
                    },
                    ArgType::Unary(rule) => {
                        let optimized = Self::optimize_rule(*rule)?;
                        Ok(Rule::Preserve(ArgType::Unary(Box::new(optimized))))
                    }
                }
            }
            Rule::Compare(op, args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Compare(op, optimized))
            },
            Rule::Logic(op, args) => {
                match args {
                    ArgType::Multiple(arr) => {
                        let optimized = Self::optimize_args(&arr)?;
                        Ok(Rule::Logic(op, ArgType::Multiple(optimized)))
                    },
                    ArgType::Unary(rule) => {
                        let optimized = Self::optimize_rule(*rule)?;
                        Ok(Rule::Logic(op, ArgType::Unary(Box::new(optimized))))
                    }
                }
            },
            Rule::Merge(ref args) => {
                let optimized = Self::optimize_args(args)?;
                Ok(Rule::Merge(optimized))
            }
            Rule::Cat(ref args) => {
                let optimized = Self::optimize_args(args)?;
                Ok(Rule::Cat(optimized))
            }
            Rule::If(ref args) => {
                match args {
                    ArgType::Multiple(arr) => {
                        let optimized = Self::optimize_args(arr)?;
                        Ok(Rule::If(ArgType::Multiple(optimized)))
                    },
                    ArgType::Unary(rule) => {
                        let r = rule.as_ref().clone();
                        let optimized = Self::optimize_rule(r)?;
                        Ok(Rule::If(ArgType::Unary(Box::new(optimized))))
                    }
                }
            }
            Rule::Ternary(ref args) => {
                let optimized = Self::optimize_args(args)?;
                Ok(Rule::Ternary(optimized))
            },
            Rule::Coalesce(ref args) => {
                let optimized = Self::optimize_args(args)?;
                Ok(Rule::Coalesce(optimized))
            },
            Rule::Throw(rule) => {
                let optimized = Self::optimize_rule(*rule)?;
                Ok(Rule::Throw(Box::new(optimized)))
            },
            Rule::Try(args) => {
                match args {
                    ArgType::Multiple(arr) => {
                        let optimized = Self::optimize_args(&arr)?;
                        Ok(Rule::Try(ArgType::Multiple(optimized)))
                    },
                    ArgType::Unary(rule) => {
                        let optimized = Self::optimize_rule(*rule)?;
                        Ok(Rule::Try(ArgType::Unary(Box::new(optimized))))
                    }
                }
            },
            Rule::Custom(name, args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Custom(name, optimized))
            },
        }
    }

    /// Parses a JSON Value into a `Rule` that can be evaluated.
    /// 
    /// This function accepts a JSONLogic-compliant expression and converts it
    /// into an internal `Rule` representation.
    /// 
    /// ## Arguments
    /// - `value`: A JSON value representing the rule.
    /// 
    /// ## Returns
    /// - `Result<Rule, Error>`: A valid rule or an error if parsing fails.
    /// 
    /// ## Example
    /// ```rust
    /// use datalogic_rs::Rule;
    /// use serde_json::json;
    /// 
    /// let rule = Rule::from_value(&json!({">": [{"var": "salary"}, 50000]})).unwrap();
    /// ```
    pub fn from_value(value: &Value) -> Result<Self, Error> {
        match value {
            Value::Object(map) if map.len() == 1 => {
                let (op, args_raw) = map.iter().next().unwrap();
                let args = match args_raw {
                    Value::Array(arr) => arr.iter()
                        .map(Rule::from_value)
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => vec![Rule::from_value(args_raw)?],
                };
                
                let rule = match op.as_str() {
                    // Variable access
                    "var" => Ok(Rule::Var(
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        args.get(1).cloned().map(Box::new)
                    )),
                    "val" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Val(arg))
                    }
                    "exists" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Exists(arg))
                    },
                    // Comparison operators
                    "==" => Ok(Rule::Compare(CompareType::Equals, args)),
                    "===" => Ok(Rule::Compare(CompareType::StrictEquals, args)),
                    "!=" => Ok(Rule::Compare(CompareType::NotEquals, args)),
                    "!==" => Ok(Rule::Compare(CompareType::StrictNotEquals, args)),
                    ">" => Ok(Rule::Compare(CompareType::GreaterThan, args)),
                    "<" => Ok(Rule::Compare(CompareType::LessThan, args)),
                    ">=" => Ok(Rule::Compare(CompareType::GreaterThanEqual, args)),
                    "<=" => Ok(Rule::Compare(CompareType::LessThanEqual, args)),
                    
                    // Logical operators
                    "and" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Logic(LogicType::And, arg))
                    },
                    "or" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Logic(LogicType::Or, arg))
                    },
                    "!" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Logic(LogicType::Not, arg))
                    },
                    "!!" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Logic(LogicType::DoubleBang, arg))
                    },
                    
                    // Control operators
                    "if" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::If(arg))
                    },
                    "?:" => Ok(Rule::Ternary(args)),
                    "??" => Ok(Rule::Coalesce(args)),
                    
                    // Array operators
                    "map" => {
                        if let Value::Array(_) = args_raw {
                            Ok(Rule::Map(
                                Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                                Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                            ))
                        } else {
                            Ok(Rule::Map(Box::new(Rule::Value(Value::Null)), Box::new(Rule::Value(Value::Null))))
                        }
                    },
                    "filter" => {
                        if let Value::Array(_) = args_raw {
                            Ok(Rule::Filter(
                                Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                                Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                            ))
                        } else {
                            Ok(Rule::Filter(Box::new(Rule::Value(Value::Null)), Box::new(Rule::Value(Value::Null))))
                        }
                    },
                    "reduce" => {
                        let array = args.first().cloned().unwrap_or(Rule::Value(Value::Null));
                        let predicate = args.get(1).cloned().unwrap_or(Rule::Value(Value::Null));
                        let initial = args.get(2).cloned().unwrap_or(Rule::Value(Value::Null));

                        // Try to desugar if predicate is a simple arithmetic operation
                        if let Rule::Arithmetic(op, ArgType::Multiple(args)) = &predicate {
                            if args.len() == 2 && is_flat_arithmetic_predicate(&predicate) {
                                let merged = Rule::Merge(vec![initial, array]);

                                // Convert to direct arithmetic operation
                                return Ok(Rule::Arithmetic(
                                    *op,
                                    ArgType::Unary(Box::new(merged))
                                ));
                            }
                        }

                        // Fall back to regular reduce if desugaring not possible
                        Ok(Rule::Reduce(
                            Box::new(array),
                            Box::new(predicate),
                            Box::new(initial)
                        ))
                    },
                    "all" if args_raw.is_array() => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::All,
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "none" if args_raw.is_array() => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::None,
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "some" if args_raw.is_array() => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::Some,
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "all" | "none" | "some" => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::Invalid,
                        Box::new(Rule::Value(Value::Null)),
                        Box::new(Rule::Value(Value::Null))
                    )),
                    "merge" => Ok(Rule::Merge(args)),
                    
                    // Missing operators
                    "missing" => Ok(Rule::Missing(args)),
                    "missing_some" => Ok(Rule::MissingSome(args)),
                    
                    // String operators
                    "in" => Ok(Rule::In(
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "cat" => Ok(Rule::Cat(args)),
                    "substr" => Ok(Rule::Substr(
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null))),
                        args.get(2).cloned().map(Box::new)
                    )),
                    
                    // Arithmetic operators
                    "+" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Add, arg))
                    },
                    "*" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Multiply, arg))
                    },
                    "-" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Subtract, arg))
                    },
                    "/" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Divide, arg))
                    },
                    "%" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Modulo, arg))
                    },
                    "max" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Max, arg))
                    },
                    "min" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Min, arg))
                    },
                    "preserve" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Preserve(arg))
                    },
                    "throw" => Ok(Rule::Throw(Box::new(args[0].clone()))),
                    "try" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Multiple(args),
                            _ => ArgType::Unary(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Try(arg))
                    },
                    _ => {
                        let json_logic = JsonLogic::global();
                        if json_logic.get_operator(op).is_some() {
                            Ok(Rule::Custom(op.to_string(), args))
                        } else {
                            Err(Error::InvalidArguments(op.to_string()))
                        }
                    },
                };
                Self::optimize_rule(rule?)
            },
            Value::Array(arr) => {
                let rule = Rule::Array(
                    arr.iter()
                        .map(Rule::from_value)
                        .collect::<Result<Vec<_>, _>>()?
                );
                Self::optimize_rule(rule)
            },
            _ => Ok(Rule::Value(value.clone())),
        }
    }

    pub fn apply<'a>(&'a self, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        match self {
            Rule::Value(value) => return Ok(Cow::Borrowed(value)),
            Rule::Array(rules) => {
                let mut results = Vec::with_capacity(rules.len());
                for rule in rules {
                    results.push(rule.apply(data)?.into_owned());
                }
                return Ok(Cow::Owned(Value::Array(results)));
            }
            Rule::Var(path, default) => VAR_OP.apply(path, default.as_deref(), data),
            Rule::Val(path) => VAL_OP.apply(path, data),
            Rule::Exists(path) => EXISTS_OP.apply(path, data),

            Rule::Map(array_rule, predicate) => MAP_OP.apply(array_rule, predicate, data),
            Rule::Filter(array_rule, predicate) => FILTER_OP.apply(array_rule, predicate, data),
            Rule::Reduce(array_rule, reducer_rule, initial_rule) => 
                REDUCE_OP.apply(array_rule, reducer_rule, initial_rule, data),
            Rule::Merge(args) => MERGE_OP.apply(args, data),
            Rule::ArrayPredicate(op, array_rule, predicate) => 
                ARRAY_PREDICATE_OP.apply(array_rule, predicate, data, op),

            Rule::Compare(op, args) => COMPARE_OP.apply(args, data, op),
            Rule::Logic(op, args) => LOGIC_OP.apply(args, data, op),
            Rule::Arithmetic(op, args) => ARITHMETIC_OP.apply(args, data, op),
    
            Rule::If(args) => IF_OP.apply(args, data),
            Rule::Ternary(args) => TERNARY_OP.apply(args, data),
            Rule::Coalesce(args) => COALESCE_OP.apply(args, data),

            Rule::In(search, target) => IN_OP.apply(search, target, data),
            Rule::Cat(args) => CAT_OP.apply(args, data),
            Rule::Substr(string, start, length) => 
                SUBSTR_OP.apply(string, start, length.as_deref(), data),

            Rule::Missing(args) => MISSING_OP.apply(args, data),
            Rule::MissingSome(args) => MISSING_SOME_OP.apply(args, data),
            
            Rule::Preserve(args) => PRESERVE_OP.apply(args, data),
            Rule::Throw(rule) => {
                let result = rule.apply(data)?;
                Err(Error::Custom(result.into_owned().to_string()))
            },
            Rule::Try(args) => TRY_OP.apply(args, data),
            Rule::Custom(name, args) => {
                let json_logic = JsonLogic::global();
                if let Some(op) = json_logic.get_operator(name) {
                    let mut evaluated_args = Vec::with_capacity(args.len());
                    for arg in args {
                        evaluated_args.push(arg.apply(data)?.into_owned());
                    }
                    op.apply(&evaluated_args, data)
                } else {
                    Err(Error::UnknownExpression(name.clone()))
                }
            }
        }
    }
}