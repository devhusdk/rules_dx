package dx.docs.javadoc;

import com.sun.source.doctree.DeprecatedTree;
import com.sun.source.doctree.DocCommentTree;
import com.sun.source.doctree.DocTree;
import com.sun.source.doctree.EntityTree;
import com.sun.source.doctree.LinkTree;
import com.sun.source.doctree.LiteralTree;
import com.sun.source.doctree.ParamTree;
import com.sun.source.doctree.ReferenceTree;
import com.sun.source.doctree.ReturnTree;
import com.sun.source.doctree.SeeTree;
import com.sun.source.doctree.SinceTree;
import com.sun.source.doctree.StartElementTree;
import com.sun.source.doctree.TextTree;
import com.sun.source.doctree.ThrowsTree;
import com.sun.source.util.DocTreePath;
import com.sun.source.util.DocTrees;
import com.sun.source.util.TreePath;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import javax.lang.model.element.Element;
import javax.lang.model.element.ExecutableElement;
import javax.lang.model.element.TypeElement;
import javax.lang.model.type.DeclaredType;
import javax.lang.model.type.TypeMirror;
import javax.lang.model.util.ElementFilter;
import javax.lang.model.util.Elements;
import javax.lang.model.util.Types;

/** Converts Javadoc comments to the Markdown the shared site renders. */
final class Docs {
  /** The parts of one resolved comment. */
  static final class Doc {
    String body = "";
    final Map<String, String> params = new LinkedHashMap<>();
    String returns;
    String deprecated;
    final List<String> throwsLines = new ArrayList<>();
    final List<String> seeLines = new ArrayList<>();
    String since;

    String markdown() {
      StringBuilder out = new StringBuilder(body);
      if (deprecated != null) {
        out.append("\n\n**Deprecated.** ").append(deprecated);
      }
      if (!throwsLines.isEmpty()) {
        out.append("\n\n**Throws**\n");
        for (String line : throwsLines) {
          out.append("\n- ").append(line);
        }
      }
      if (!seeLines.isEmpty()) {
        out.append("\n\n**See also**\n");
        for (String line : seeLines) {
          out.append("\n- ").append(line);
        }
      }
      if (since != null) {
        out.append("\n\n**Since:** ").append(since);
      }
      return tidy(out.toString());
    }
  }

  private final DocTrees trees;
  private final Names names;
  private final Elements elements;
  private final Types types;

  Docs(DocTrees trees, Names names, Elements elements, Types types) {
    this.trees = trees;
    this.names = names;
    this.elements = elements;
    this.types = types;
  }

  /** Resolves the documentation of one element, inheriting from overridden methods. */
  Doc of(Element element) {
    DocCommentTree comment = trees.getDocCommentTree(element);
    TreePath path = trees.getPath(element);
    Doc doc = new Doc();
    ExecutableElement overridden = null;
    Doc inherited = null;
    if (element instanceof ExecutableElement) {
      overridden = overridden((ExecutableElement) element);
      if (overridden != null) {
        inherited = of(overridden);
      }
    }
    if (comment != null && path != null) {
      Renderer renderer = new Renderer(new DocTreePath(path, comment), inherited);
      doc.body = renderer.render(comment.getFullBody());
      for (DocTree tag : comment.getBlockTags()) {
        block(renderer, doc, tag);
      }
    }
    if (inherited != null) {
      if (doc.body.isEmpty()) {
        doc.body = inherited.body;
      }
      for (Map.Entry<String, String> param : inherited.params.entrySet()) {
        doc.params.putIfAbsent(param.getKey(), param.getValue());
      }
      if (doc.returns == null) {
        doc.returns = inherited.returns;
      }
      if (comment == null) {
        doc.deprecated = inherited.deprecated;
        doc.throwsLines.addAll(inherited.throwsLines);
        doc.seeLines.addAll(inherited.seeLines);
        doc.since = inherited.since;
      }
    }
    return doc;
  }

  private void block(Renderer renderer, Doc doc, DocTree tag) {
    switch (tag.getKind()) {
      case PARAM:
        ParamTree param = (ParamTree) tag;
        if (!param.isTypeParameter()) {
          doc.params.put(
              param.getName().getName().toString(), renderer.render(param.getDescription()));
        }
        break;
      case RETURN:
        doc.returns = renderer.render(((ReturnTree) tag).getDescription());
        break;
      case THROWS:
      case EXCEPTION:
        ThrowsTree thrown = (ThrowsTree) tag;
        String description = renderer.render(thrown.getDescription());
        doc.throwsLines.add(
            "`"
                + thrown.getExceptionName().getSignature()
                + "`"
                + (description.isEmpty() ? "" : ": " + description));
        break;
      case DEPRECATED:
        doc.deprecated = renderer.render(((DeprecatedTree) tag).getBody());
        break;
      case SEE:
        doc.seeLines.add(renderer.render(((SeeTree) tag).getReference()));
        break;
      case SINCE:
        doc.since = renderer.render(((SinceTree) tag).getBody());
        break;
      default:
        break;
    }
  }

  private ExecutableElement overridden(ExecutableElement method) {
    Element owner = method.getEnclosingElement();
    if (!(owner instanceof TypeElement)
        || method.getModifiers().contains(javax.lang.model.element.Modifier.STATIC)) {
      return null;
    }
    return search(method, (TypeElement) owner, (TypeElement) owner, new TreeSet<>());
  }

  private ExecutableElement search(
      ExecutableElement method, TypeElement owner, TypeElement from, Set<String> seen) {
    for (TypeMirror supertype : types.directSupertypes(from.asType())) {
      if (!(supertype instanceof DeclaredType)) {
        continue;
      }
      TypeElement parent = (TypeElement) ((DeclaredType) supertype).asElement();
      if (!seen.add(parent.getQualifiedName().toString())) {
        continue;
      }
      for (ExecutableElement candidate : ElementFilter.methodsIn(parent.getEnclosedElements())) {
        if (elements.overrides(method, candidate, owner)) {
          return candidate;
        }
      }
      ExecutableElement deeper = search(method, owner, parent, seen);
      if (deeper != null) {
        return deeper;
      }
    }
    return null;
  }

  /** Collapses blank runs and trims trailing spaces. */
  static String tidy(String text) {
    StringBuilder out = new StringBuilder();
    int blank = 0;
    for (String line : text.split("\n", -1)) {
      String trimmed = line.replaceAll("[ \t]+$", "");
      if (trimmed.isEmpty()) {
        blank++;
        continue;
      }
      if (out.length() > 0) {
        out.append(blank > 0 ? "\n\n" : "\n");
      }
      blank = 0;
      out.append(trimmed);
    }
    return out.toString();
  }

  private static String escape(String text) {
    StringBuilder out = new StringBuilder();
    for (int index = 0; index < text.length(); index++) {
      char ch = text.charAt(index);
      if (ch == '\\' || ch == '`' || ch == '*' || ch == '_' || ch == '[' || ch == ']') {
        out.append('\\').append(ch);
      } else if (ch == '<') {
        out.append("&lt;");
      } else if (ch == '>') {
        out.append("&gt;");
      } else {
        out.append(ch);
      }
    }
    return out.toString();
  }

  private static String code(String text) {
    String flat = text.replaceAll("\\s*\n\\s*", " ").trim();
    int longest = 0;
    int run = 0;
    for (int index = 0; index < flat.length(); index++) {
      run = flat.charAt(index) == '`' ? run + 1 : 0;
      longest = Math.max(longest, run);
    }
    String fence = "`".repeat(longest + 1);
    String pad = flat.startsWith("`") || flat.endsWith("`") ? " " : "";
    return fence + pad + flat + pad + fence;
  }

  private static String entity(String name) {
    switch (name) {
      case "lt":
        return "<";
      case "gt":
        return ">";
      case "amp":
        return "&";
      case "quot":
        return "\"";
      case "apos":
        return "'";
      case "nbsp":
        return " ";
      default:
        return "&" + name + ";";
    }
  }

  /** Renders inline doc trees of one comment as Markdown. */
  private final class Renderer {
    private final DocTreePath root;
    private final Doc inherited;
    private String out = "";
    private String pre;
    private final Deque<Boolean> lists = new ArrayDeque<>();

    Renderer(DocTreePath root, Doc inherited) {
      this.root = root;
      this.inherited = inherited;
    }

    String render(List<? extends DocTree> nodes) {
      String saved = out;
      out = "";
      for (DocTree node : nodes) {
        node(node);
      }
      String result = tidy(out);
      out = saved;
      return result;
    }

    private void append(String text) {
      if (pre != null) {
        pre += text;
      } else {
        out += text;
      }
    }

    private void node(DocTree node) {
      switch (node.getKind()) {
        case TEXT:
          text(((TextTree) node).getBody());
          break;
        case ENTITY:
          String entity = entity(((EntityTree) node).getName().toString());
          append(pre != null ? entity : escape(entity));
          break;
        case START_ELEMENT:
          start((StartElementTree) node);
          break;
        case END_ELEMENT:
          end(((com.sun.source.doctree.EndElementTree) node).getName().toString());
          break;
        case CODE:
        case LITERAL:
          String body = ((LiteralTree) node).getBody().getBody();
          append(pre != null ? body : code(body));
          break;
        case LINK:
        case LINK_PLAIN:
          link((LinkTree) node);
          break;
        case REFERENCE:
          append(reference((ReferenceTree) node, null, node));
          break;
        case INHERIT_DOC:
          append(inherited == null ? "" : inherited.body);
          break;
        case COMMENT:
          break;
        default:
          append(escape(node.toString()));
          break;
      }
    }

    private void text(String body) {
      if (pre != null) {
        pre += body;
        return;
      }
      String text = body.replaceAll("\n[ \t]+", "\n");
      if (out.isEmpty() || Character.isWhitespace(out.charAt(out.length() - 1))) {
        text = text.replaceAll("^\\s+", "");
      }
      out += escape(text);
    }

    private void start(StartElementTree tree) {
      String name = tree.getName().toString().toLowerCase(Locale.ROOT);
      switch (name) {
        case "p":
          append(pre != null ? "" : "\n\n");
          break;
        case "br":
          append(pre != null ? "\n" : "  \n");
          break;
        case "code":
          append(pre != null ? "" : "`");
          break;
        case "pre":
          pre = "";
          break;
        case "ul":
        case "ol":
          lists.push("ol".equals(name));
          append("\n\n");
          break;
        case "li":
          if (pre == null && !out.endsWith("\n\n")) {
            out = out.replaceAll("\\s+$", "");
          }
          append(lists.isEmpty() || !lists.peek() ? "\n- " : "\n1. ");
          break;
        case "em":
        case "i":
          append("*");
          break;
        case "strong":
        case "b":
          append("**");
          break;
        default:
          append(pre != null ? "" : escape(tree.toString()));
          break;
      }
    }

    private void end(String raw) {
      String name = raw.toLowerCase(Locale.ROOT);
      switch (name) {
        case "p":
        case "li":
        case "br":
          break;
        case "code":
          append(pre != null ? "" : "`");
          break;
        case "pre":
          if (pre != null) {
            out += "\n\n```\n" + dedent(pre) + "\n```\n\n";
            pre = null;
          }
          break;
        case "ul":
        case "ol":
          if (!lists.isEmpty()) {
            lists.pop();
          }
          append("\n\n");
          break;
        case "em":
        case "i":
          append("*");
          break;
        case "strong":
        case "b":
          append("**");
          break;
        default:
          append(pre != null ? "" : escape("</" + raw + ">"));
          break;
      }
    }

    private String dedent(String block) {
      String[] lines = block.replaceAll("^\\n+|\\s+$", "").split("\n", -1);
      int indent = Integer.MAX_VALUE;
      for (String line : lines) {
        if (!line.trim().isEmpty()) {
          int spaces = 0;
          while (spaces < line.length() && line.charAt(spaces) == ' ') {
            spaces++;
          }
          indent = Math.min(indent, spaces);
        }
      }
      StringBuilder result = new StringBuilder();
      for (int index = 0; index < lines.length; index++) {
        if (index > 0) {
          result.append('\n');
        }
        String line = lines[index];
        result.append(
            line.length() >= indent && indent != Integer.MAX_VALUE
                ? line.substring(indent)
                : line.trim());
      }
      return result.toString();
    }

    private void link(LinkTree tree) {
      List<? extends DocTree> label = tree.getLabel();
      String shown = label.isEmpty() ? null : render(label);
      append(reference(tree.getReference(), shown, tree));
    }

    private String reference(ReferenceTree reference, String shown, DocTree parent) {
      String signature = reference.getSignature();
      int hash = signature.indexOf('#');
      String owner = hash < 0 ? signature : signature.substring(0, hash);
      String member = hash < 0 ? "" : signature.substring(hash + 1);
      String label = shown;
      if (label == null) {
        String simple = owner.substring(owner.lastIndexOf('.') + 1);
        label = simple + (simple.isEmpty() || member.isEmpty() ? "" : ".") + member;
        label = label.replace("[", "").replace("]", "");
      }
      Element target = trees.getElement(new DocTreePath(new DocTreePath(root, parent), reference));
      String key;
      if (target == null
          || target.asType() == null
          || "TYPE_PARAMETER".equals(target.getKind().name())) {
        key = signature.replace('#', '.');
        if (key.startsWith(".")) {
          key = key.substring(1);
        }
      } else {
        key = names.key(target);
      }
      return "[" + label.replace("\n", " ") + "](jvm:" + key + ")";
    }
  }
}
