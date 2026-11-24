config {
  call_module_type = "none"
}

plugin "terraform" {
  enabled = true
  preset  = "all"
}

rule "terraform_module_pinned_source" {
  enabled = true
}

rule "terraform_required_providers" {
  enabled = true
}

rule "terraform_required_version" {
  enabled = true
}

rule "terraform_unused_declarations" {
  enabled = true
}
