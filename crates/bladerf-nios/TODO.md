- Assert and trow error, if in retune packet the maximum width of 
  nint and nfrac is reached
- Instead of asserts, we could user normal throwing of errors. This has the benefit 
  of allowing the application to decide how to handle such an error
- Test boundries of fields in the tests e.g. limit of bits is not exceeded.
- Fully implement checks in packet validation() methods
- Use thiserror instead of anyhow (e.g. for success() -> Result<> method on response packet)
- 