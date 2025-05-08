- In packet.rs do composition (Struct : OtherStuct notation) to 
  first build a generic packet, that does provide basic fields like flags magic etc,
  which are present in every NiosPacket Request and Response. 
  Then, have the additional fields like addr and data implemented depending 
  on Requet/Response and addr and data width.
- If this struct notation does not work, we could create a new type that 
  references the base Generic type but implements additional methods
- Assert and trow error, if in retune packet the maximum width of 
  nint and nfrac is reached
- Instead of asserts, we could user normal throwing of errors. This has the benefit 
  of allowing the application to decide how to handle such an error
- Create a crate that contains definitions of macros o enums, which are used everywhere in the code
  (Also in subcrates)
- Idea: Implement all methods for Generic Packet, even if they do not amke sense (like timestamp etc). Then only add supported methods in the Subtyped packets like e.g. RetuneREquest and RetuneResponse