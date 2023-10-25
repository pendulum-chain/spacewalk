## Highlights

[ADD_BULLETPOINTS_HERE]

{{ range .Versions }}
<a name="{{ .Tag.Name }}"></a>

## {{ if .Tag.Previous }}[{{ .Tag.Name }}]({{ $.Info.RepositoryURL }}/compare/{{ .Tag.Previous.Name }}...{{ .Tag.Name }}){{ else }}{{ .Tag.Name }}{{ end }} ({{ datetime "2006-01-02" .Tag.Date }})

{{ range .CommitGroups -}}

### {{ .Title }}

{{ range .MergeCommits -}}

{{ .TrimmedBody }} [#{{ .Merge.Source }}](https://github.com/interlay/interbtc/issues/{{ .Merge.Source }}) {{ end }} {{ end -}}
{{ range .CommitGroups -}}

{{ range .Commits -}}

* {{ .Subject }}
  {{ end }}
  {{ end -}}

{{- if .NoteGroups -}}
{{ range .NoteGroups -}}

### {{ .Title }}

{{ range .Notes }}
{{ .Body }}
{{ end }}
{{ end -}}
{{ end -}}
{{ end -}}