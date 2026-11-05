config {
  call_module_type = "none"
}

plugin "terraform" {
  enabled = true
  preset  = "all"
}

rule "terraform_unused_declarations" {
  enabled = true
}
