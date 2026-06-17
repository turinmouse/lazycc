{% for function_name in function_names %}
unfunction {{ function_name }} 2>/dev/null || true
{% endfor %}
{% for env_name in unset_envs %}
unset {{ env_name }}
{% endfor %}
{% for export in exports %}
export {{ export.name }}={{ export.value }}
{% endfor %}
{% if codex_wrapper %}
codex() {
  command codex \
{% for config_arg in codex_wrapper.config_args %}
    -c {{ config_arg }} \
{% endfor %}
    "$@"
}
{% endif %}
lazycc() {
  command lazycc "$@"
  local lazycc_status=$?

  if [ $lazycc_status -eq 0 ] && { [ $# -eq 0 ] || [ "$1" = "use" ]; }; then
    eval "$(command lazycc init zsh)"
  elif [ $lazycc_status -eq 0 ] && [ "$1" = "tui" ]; then
    eval "$(command lazycc init zsh)"
  fi

  return $lazycc_status
}
