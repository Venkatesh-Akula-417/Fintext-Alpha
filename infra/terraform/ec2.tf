# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — EC2 Application Host & Elastic IP (EIP)
# ══════════════════════════════════════════════════════════════════════════════
# Single-node Docker Compose production host in us-east-1 public subnet:
# - c6i.xlarge (4 vCPU, 8 GiB RAM) compute-optimized architecture
# - 100 GB gp3 encrypted root block storage
# - Static public Elastic IP for predictable DNS routing & partner IP whitelisting
# ══════════════════════════════════════════════════════════════════════════════

data "aws_ami" "ubuntu" {
  most_recent = true

  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-*"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }

  owners = ["099720109477"] # Canonical
}

resource "aws_instance" "fintext_api" {
  ami                    = data.aws_ami.ubuntu.id
  instance_type          = var.ec2_instance_type
  subnet_id              = aws_subnet.public[0].id
  vpc_security_group_ids = [aws_security_group.ec2_sg.id]
  iam_instance_profile   = aws_iam_instance_profile.ec2_profile.name

  root_block_device {
    volume_type           = "gp3"
    volume_size           = var.ec2_root_volume_size_gb
    encrypted             = true
    delete_on_termination = true

    tags = {
      Name = "fintext-api-root-gp3-${var.environment}"
      Tier = "Storage-OS"
    }
  }

  user_data = <<-EOF
              #!/bin/bash
              set -ex

              # 1. Update OS packages and prerequisites
              export DEBIAN_FRONTEND=noninteractive
              apt-get update -y
              apt-get upgrade -y
              apt-get install -y ca-certificates curl gnupg lsb-release git unzip jq awscli

              # 2. Install Docker Community Engine & Docker Compose Plugin
              install -m 0755 -d /etc/apt/keyrings
              curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
              chmod a+r /etc/apt/keyrings/docker.gpg
              echo \
                "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
                $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
              apt-get update -y
              apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

              systemctl enable docker
              systemctl start docker
              usermod -aG docker ubuntu

              # 3. Kernel tuning for high-throughput institutional quant traffic
              cat << 'SYSCTL_EOF' > /etc/sysctl.d/99-fintext.conf
              net.core.somaxconn = 65535
              net.ipv4.tcp_max_syn_backlog = 65535
              net.ipv4.ip_local_port_range = 1024 65535
              net.ipv4.tcp_tw_reuse = 1
              fs.file-max = 2097152
              SYSCTL_EOF
              sysctl --system

              # 4. Create deployment directories
              mkdir -p /opt/fintext/logs /opt/fintext/backups /opt/fintext/models
              chown -R ubuntu:ubuntu /opt/fintext

              echo "FinText Alpha Vectorizer Host Provisioning Complete" > /opt/fintext/provisioned.log
              EOF

  tags = {
    Name = "fintext-api-host-${var.environment}"
    Tier = "Compute-Application"
  }
}

# ── Static Elastic IP Assignment ─────────────────────────────────────────────

resource "aws_eip" "fintext_api_eip" {
  domain   = "vpc"
  instance = aws_instance.fintext_api.id

  depends_on = [aws_internet_gateway.fintext_igw]

  tags = {
    Name = "fintext-api-eip-${var.environment}"
    Tier = "Network-EIP"
  }
}

# ── Staged GA Multi-AZ Compute & Ingress (Default Disabled) ──────────────────

resource "aws_instance" "fintext_api_secondary" {
  count                  = var.ha_compute_enabled ? 1 : 0
  ami                    = data.aws_ami.ubuntu.id
  instance_type          = var.ec2_instance_type
  subnet_id              = aws_subnet.public[1].id
  vpc_security_group_ids = [aws_security_group.ec2_sg.id]
  iam_instance_profile   = aws_iam_instance_profile.ec2_profile.name

  root_block_device {
    volume_type           = "gp3"
    volume_size           = var.ec2_root_volume_size_gb
    encrypted             = true
    delete_on_termination = true

    tags = {
      Name = "fintext-api-secondary-root-gp3-${var.environment}"
      Tier = "Storage-OS"
    }
  }

  tags = {
    Name = "fintext-api-secondary-host-${var.environment}"
    Tier = "Compute-Application-Secondary"
  }
}

resource "aws_lb" "fintext_alb" {
  count              = var.alb_enabled ? 1 : 0
  name               = "fintext-alb-${var.environment}"
  internal           = false
  load_balancer_type = "application"
  security_groups    = [aws_security_group.ec2_sg.id]
  subnets            = aws_subnet.public[*].id

  enable_deletion_protection = false

  tags = {
    Name = "fintext-alb-${var.environment}"
    Tier = "Ingress-ALB"
  }
}
