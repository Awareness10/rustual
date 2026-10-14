//! TCSS tokenizer. Handles comments (`/* */`), selectors, `{` `}`, nesting with `&`,
//! declarations, `$var: value;` definitions and `$var` references.
//! Internal API: the parser is the only consumer; shape it as needed.
