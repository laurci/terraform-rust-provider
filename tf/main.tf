terraform {
  required_providers {
    ubicloud = {
      source  = "ubicloud/ubicloud"
      version = ">= 1.0"
    }

    namecheap = {
      source = "namecheap/namecheap"
      version = ">= 2.0.0"
    }

    tls = {
      source = "hashicorp/tls"
      version = ">= 1.0"
    }

    ansible = {
      source = "ansible/ansible"
      version = "1.1.0"
    }

    random = {
      source = "hashicorp/random"
      version = "3.6.0"
    }

    wireguard = {
      source = "OJFord/wireguard"
      version = "0.2.2"
    }
  }
}

variable "ubicloud_email" {
  type      = string
  nullable  = false
  sensitive = true
}

variable "ubicloud_password" {
  type      = string
  nullable  = false
  sensitive = true
}

variable "ubicloud_project" {
  type      = string
  nullable  = false
}

variable "namecheap_user" {
  type      = string
  nullable  = false
}

variable "namecheap_api_key" {
  type      = string
  nullable  = false
  sensitive = true
}

variable "root_domain" {
  type      = string
  nullable  = false
}

variable "subdomain" {
  type      = string
  nullable  = false
  default   = "ubicloud-demo"
}

locals {
  dns_ttl = 60
  cidr = "10.112.0.x"
}

provider "ubicloud" {
  email    = var.ubicloud_email
  password = var.ubicloud_password
}

provider "namecheap" {
  user_name = var.namecheap_user
  api_user = var.namecheap_user
  api_key = var.namecheap_api_key
}

resource "tls_private_key" "ssh_key" {
  algorithm = "RSA"
  rsa_bits  = 4096
}

data "tls_public_key" "ssh_key" {
  private_key_pem = tls_private_key.ssh_key.private_key_pem
}

resource "random_password" "k8s_token" {
  length           = 16
  special          = false
}

resource "wireguard_asymmetric_key" "master" { }
resource "wireguard_asymmetric_key" "worker1" { }
resource "wireguard_asymmetric_key" "worker2" { }

resource "ubicloud_vm" "master" {
  region             = "hetzner-hel1"
  project_id         = var.ubicloud_project
  name               = "terraform-k8s-master"
  size               = "standard-4"
  image              = "ubuntu-jammy"
  user               = "kube"
  public_key         = data.tls_public_key.ssh_key.public_key_openssh
  enable_public_ipv4 = true
}

resource "ubicloud_vm" "worker1" {
  region             = "hetzner-hel1"
  project_id         = var.ubicloud_project
  name               = "terraform-k8s-worker1"
  size               = "standard-4"
  image              = "ubuntu-jammy"
  user               = "kube"
  public_key         = data.tls_public_key.ssh_key.public_key_openssh
  enable_public_ipv4 = true
}

resource "ubicloud_vm" "worker2" {
  region             = "hetzner-hel1"
  project_id         = var.ubicloud_project
  name               = "terraform-k8s-worker2"
  size               = "standard-4"
  image              = "ubuntu-jammy"
  user               = "kube"
  public_key         = data.tls_public_key.ssh_key.public_key_openssh
  enable_public_ipv4 = true
}


resource "namecheap_domain_records" "root-domain" {
  domain = var.root_domain
  
  record {
    type      = "A"
    hostname  = var.subdomain
    address   = ubicloud_vm.master.public_ipv4
    ttl       = local.dns_ttl
  }

  record {
    type      = "A"
    hostname  = var.subdomain
    address   = ubicloud_vm.worker1.public_ipv4
    ttl       = local.dns_ttl
  }

  record {
    type      = "A"
    hostname  = var.subdomain
    address   = ubicloud_vm.worker2.public_ipv4
    ttl       = local.dns_ttl
  }

  record {
    type      = "A"
    hostname  = "*.${var.subdomain}"
    address   = ubicloud_vm.master.public_ipv4
    ttl       = local.dns_ttl
  }

  record {
    type      = "A"
    hostname  = "*.${var.subdomain}"
    address   = ubicloud_vm.worker1.public_ipv4
    ttl       = local.dns_ttl
  }

    record {
    type      = "A"
    hostname  = "*.${var.subdomain}"
    address   = ubicloud_vm.worker2.public_ipv4
    ttl       = local.dns_ttl
  }
}


resource "ansible_host" "master" {
  name   = ubicloud_vm.master.public_ipv4
  groups = ["master"]

  variables = {
    ansible_user                 = ubicloud_vm.master.user,
    ansible_ssh_private_key_file = "./temp.pem",
    k8s_token                    = random_password.k8s_token.result,
    self_public_ip               = ubicloud_vm.master.public_ipv4,
    master_public_ip             = ubicloud_vm.master.public_ipv4,
    cidr                         = replace(local.cidr, "x", 0),
    internal_ip                  = replace(local.cidr, "x", 1),
    wg_self_private_key          = wireguard_asymmetric_key.master.private_key,
    wg_self_public_key           = wireguard_asymmetric_key.master.public_key,
    wg_worker1_public_key        = wireguard_asymmetric_key.worker1.public_key,
    wg_worker1_internal_ip       = replace(local.cidr, "x", 50),
    wg_worker2_public_key        = wireguard_asymmetric_key.worker2.public_key,
    wg_worker2_internal_ip       = replace(local.cidr, "x", 51),
  }
}

resource "ansible_host" "worker1" {
  name   = ubicloud_vm.worker1.public_ipv4
  groups = ["worker"]

  variables = {
    ansible_user                 = ubicloud_vm.worker1.user,
    ansible_ssh_private_key_file = "./temp.pem",
    k8s_token                    = random_password.k8s_token.result,
    k8s_worker_label             = "worker1",
    self_public_ip               = ubicloud_vm.worker1.public_ipv4,
    master_public_ip             = ubicloud_vm.master.public_ipv4,
    cidr                         = replace(local.cidr, "x", 0),
    master_internal_ip           = replace(local.cidr, "x", 1),
    internal_ip                  = replace(local.cidr, "x", 2),
    wg_internal_ip               = replace(local.cidr, "x", 50),
    wg_self_private_key          = wireguard_asymmetric_key.worker1.private_key,
    wg_self_public_key           = wireguard_asymmetric_key.worker1.public_key,
    wg_master_public_key         = wireguard_asymmetric_key.master.public_key,
  }
}

resource "ansible_host" "worker2" {
  name   = ubicloud_vm.worker2.public_ipv4
  groups = ["worker"]

  variables = {
    ansible_user                 = ubicloud_vm.worker2.user,
    ansible_ssh_private_key_file = "./temp.pem",
    k8s_token                    = random_password.k8s_token.result,
    k8s_worker_label             = "worker2",
    self_public_ip               = ubicloud_vm.worker2.public_ipv4,
    master_public_ip             = ubicloud_vm.master.public_ipv4,
    cidr                         = replace(local.cidr, "x", 0),
    master_internal_ip           = replace(local.cidr, "x", 1),
    internal_ip                  = replace(local.cidr, "x", 3),
    wg_internal_ip               = replace(local.cidr, "x", 51),
    wg_self_private_key          = wireguard_asymmetric_key.worker2.private_key,
    wg_self_public_key           = wireguard_asymmetric_key.worker2.public_key,
    wg_master_public_key         = wireguard_asymmetric_key.master.public_key,
  }
}

output "ssh_private_key" {
  description = "The private key to connect to the VMs"
  sensitive   = true
  value       = tls_private_key.ssh_key.private_key_pem
}

output "master_public_ip" {
  description = "The ip of the master VM"
  value       = ubicloud_vm.master.public_ipv4
}

output "worker1_public_ip" {
  description = "The ip of the worker1 VM"
  value       = ubicloud_vm.worker1.public_ipv4
}

output "worker2_public_ip" {
  description = "The ip of the worker2 VM"
  value       = ubicloud_vm.worker2.public_ipv4
}

output "root_user" {
  description = "The user to connect to the VM"
  value       = ubicloud_vm.master.user
}

output "k8s_token" {
  description = "The token to join the cluster"
  sensitive   = true
  value       = random_password.k8s_token.result
}