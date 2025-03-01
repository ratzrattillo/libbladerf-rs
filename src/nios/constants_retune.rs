// #[allow(dead_code)]

// /* Specify this value instead of a timestamp to clear the retune queue */
// #define NIOS_PKT_RETUNE_CLEAR_QUEUE ((uint64_t) -1)
//
//
// #define NIOS_PKT_RETUNE_IDX_MAGIC    0
// #define NIOS_PKT_RETUNE_IDX_TIME     1
// #define NIOS_PKT_RETUNE_IDX_INTFRAC  9
// #define NIOS_PKT_RETUNE_IDX_FREQSEL  13
// #define NIOS_PKT_RETUNE_IDX_BANDSEL  14
// #define NIOS_PKT_RETUNE_IDX_RESV     15
//
// #define NIOS_PKT_RETUNE_MAGIC        'T'
//
//
// #define FLAG_QUICK_TUNE (1 << 6)
// #define FLAG_RX (1 << 6)
// #define FLAG_TX (1 << 7)
// #define FLAG_LOW_BAND (1 << 7)
//
//
// /* Denotes no tune word is supplied. */
// #define NIOS_PKT_RETUNE_NO_HINT      0xff
//
// /* Denotes that the retune should not be scheduled - it should occur "now" */
// #define NIOS_PKT_RETUNE_NOW          ((uint64_t) 0x00)
//
// #define PACK_TXRX_FREQSEL(module_, freqsel_) (freqsel_ & 0x3f)
//
//
//
// #define NIOS_PKT_RETUNE2_IDX_MAGIC        0
// #define NIOS_PKT_RETUNE2_IDX_TIME         1
// #define NIOS_PKT_RETUNE2_IDX_NIOS_PROFILE 9
// #define NIOS_PKT_RETUNE2_IDX_RFFE_PROFILE 11
// #define NIOS_PKT_RETUNE2_IDX_RFFE_PORT    12
// #define NIOS_PKT_RETUNE2_IDX_SPDT         13
// #define NIOS_PKT_RETUNE2_IDX_RESV         14
//
// #define NIOS_PKT_RETUNE2_MAGIC            'U'
//
// /* Specify this value instead of a timestamp to clear the retune2 queue */
// #define NIOS_PKT_RETUNE2_CLEAR_QUEUE      ((uint64_t) -1)
//
// /* Denotes that the retune2 should not be scheduled - it should occur "now" */
// #define NIOS_PKT_RETUNE2_NOW              ((uint64_t) 0x00)
//
// /* The IS_RX bit embedded in the 'port' parameter of the retune2 packet */
// #define NIOS_PKT_RETUNE2_PORT_IS_RX_MASK  (0x1 << 7)
