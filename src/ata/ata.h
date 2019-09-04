#ifndef ATA_H
# define ATA_H

# include <kernel.h>

# define ATA_PRIMARY_BUS	0x1f0
# define ATA_PRIMARY_CTRL	0x3f6
# define ATA_SECONDARY_BUS	0x170
# define ATA_SECONDARY_CTRL	0x376

# define ATA_DATA_REG			0x0
# define ATA_ERROR_REG			0x1
# define ATA_FEATURES_REG		0x1
# define ATA_SECTOR_COUNT_REG	0x2
# define ATA_SECTOR_NUMBER_REG	0x3
# define ATA_CYLINDER_LOW_REG	0x4
# define ATA_CYLINDER_HIGH_REG	0x5
# define ATA_DRIVE_REG			0x6
# define ATA_STATUS_REG			0x7
# define ATA_COMMAND_REG		0x7

# define ATA_CTRL_ALTERNATE_STATUS_REG	0x0
# define ATA_CTRL_DEVICE_CONTROL_REG	0x0
# define ATA_CTRL_DRIVE_ADDRESS_REG		0x1

# define ATA_ERR_AMNF	0b00000001
# define ATA_ERR_TKZNF	0b00000010
# define ATA_ERR_ABRT	0b00000100
# define ATA_ERR_MCR	0b00001000
# define ATA_ERR_IDNF	0b00010000
# define ATA_ERR_MC		0b00100000
# define ATA_ERR_UNC	0b01000000
# define ATA_ERR_BBK	0b10000000

# define ATA_STATUS_ERR		0b00000001
# define ATA_STATUS_IDX		0b00000010
# define ATA_STATUS_CORR	0b00000100
# define ATA_STATUS_DRQ		0b00001000
# define ATA_STATUS_SRV		0b00010000
# define ATA_STATUS_DF		0b00100000
# define ATA_STATUS_RDY		0b01000000
# define ATA_STATUS_BSY		0b10000000

# define ATA_CMD_IDENTIFY	0xec

# define ATA_SECTOR_SIZE	0x200

void ata_init(void);
void ata_reset(uint16_t ctrl_bus);

#endif
