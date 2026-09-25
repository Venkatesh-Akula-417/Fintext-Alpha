# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — Virtual Private Cloud (VPC) & Subnet Architecture
# ══════════════════════════════════════════════════════════════════════════════
# Isolated dual-AZ VPC topology in us-east-1:
# - 2 Public Subnets for Internet-facing EC2 reverse proxy & API gateway
# - 2 Private Subnets strictly isolated for RDS TimescaleDB
# ══════════════════════════════════════════════════════════════════════════════

resource "aws_vpc" "fintext_vpc" {
  cidr_block           = var.vpc_cidr
  enable_dns_support   = true
  enable_dns_hostnames = true

  tags = {
    Name = "fintext-vpc-${var.environment}"
    Tier = "Network-Core"
  }
}

# ── Internet Gateway ─────────────────────────────────────────────────────────

resource "aws_internet_gateway" "fintext_igw" {
  vpc_id = aws_vpc.fintext_vpc.id

  tags = {
    Name = "fintext-igw-${var.environment}"
    Tier = "Network-Edge"
  }
}

# ── Public Subnets (Dual-AZ Ingress) ──────────────────────────────────────────

resource "aws_subnet" "public" {
  count                   = length(var.public_subnet_cidrs)
  vpc_id                  = aws_vpc.fintext_vpc.id
  cidr_block              = var.public_subnet_cidrs[count.index]
  availability_zone       = var.availability_zones[count.index]
  map_public_ip_on_launch = true

  tags = {
    Name = "fintext-public-${var.availability_zones[count.index]}-${var.environment}"
    Tier = "Public-Ingress"
  }
}

# ── Private Subnets (Dual-AZ RDS TimescaleDB Data Tier) ──────────────────────

resource "aws_subnet" "private" {
  count                   = length(var.private_subnet_cidrs)
  vpc_id                  = aws_vpc.fintext_vpc.id
  cidr_block              = var.private_subnet_cidrs[count.index]
  availability_zone       = var.availability_zones[count.index]
  map_public_ip_on_launch = false

  tags = {
    Name = "fintext-private-${var.availability_zones[count.index]}-${var.environment}"
    Tier = "Private-DataTier"
  }
}

# ── Routing Tables & Associations ────────────────────────────────────────────

resource "aws_route_table" "public" {
  vpc_id = aws_vpc.fintext_vpc.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.fintext_igw.id
  }

  tags = {
    Name = "fintext-public-rt-${var.environment}"
    Tier = "Routing-Public"
  }
}

resource "aws_route_table_association" "public" {
  count          = length(var.public_subnet_cidrs)
  subnet_id      = aws_subnet.public[count.index].id
  route_table_id = aws_route_table.public.id
}

resource "aws_route_table" "private" {
  vpc_id = aws_vpc.fintext_vpc.id

  tags = {
    Name = "fintext-private-rt-${var.environment}"
    Tier = "Routing-Private"
  }
}

resource "aws_route_table_association" "private" {
  count          = length(var.private_subnet_cidrs)
  subnet_id      = aws_subnet.private[count.index].id
  route_table_id = aws_route_table.private.id
}
