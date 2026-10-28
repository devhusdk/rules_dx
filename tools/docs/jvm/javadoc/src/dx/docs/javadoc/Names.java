package dx.docs.javadoc;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import javax.lang.model.element.Element;
import javax.lang.model.element.ElementKind;
import javax.lang.model.element.ExecutableElement;
import javax.lang.model.element.PackageElement;
import javax.lang.model.element.TypeElement;
import javax.lang.model.element.TypeParameterElement;
import javax.lang.model.element.VariableElement;
import javax.lang.model.type.ArrayType;
import javax.lang.model.type.DeclaredType;
import javax.lang.model.type.TypeKind;
import javax.lang.model.type.TypeMirror;
import javax.lang.model.type.WildcardType;
import javax.lang.model.util.Types;

/** Prints types for signatures and builds the symbol keys shared with {@code jvm_convert}. */
final class Names {
  private final Types types;

  Names(Types types) {
    this.types = types;
  }

  /** Prints a type with simple names and its type arguments. */
  String type(TypeMirror mirror) {
    switch (mirror.getKind()) {
      case DECLARED:
        {
          DeclaredType declared = (DeclaredType) mirror;
          StringBuilder out = new StringBuilder(declared.asElement().getSimpleName());
          List<? extends TypeMirror> arguments = declared.getTypeArguments();
          if (!arguments.isEmpty()) {
            out.append('<');
            for (int index = 0; index < arguments.size(); index++) {
              if (index > 0) {
                out.append(", ");
              }
              out.append(type(arguments.get(index)));
            }
            out.append('>');
          }
          return out.toString();
        }
      case ARRAY:
        return type(((ArrayType) mirror).getComponentType()) + "[]";
      case TYPEVAR:
        return mirror.toString();
      case WILDCARD:
        {
          WildcardType wildcard = (WildcardType) mirror;
          if (wildcard.getExtendsBound() != null) {
            return "? extends " + type(wildcard.getExtendsBound());
          }
          if (wildcard.getSuperBound() != null) {
            return "? super " + type(wildcard.getSuperBound());
          }
          return "?";
        }
      case VOID:
        return "void";
      case BOOLEAN:
      case BYTE:
      case SHORT:
      case INT:
      case LONG:
      case CHAR:
      case FLOAT:
      case DOUBLE:
        return mirror.getKind().name().toLowerCase(Locale.ROOT);
      default:
        return mirror.toString();
    }
  }

  /** Prints the type parameters of a declaration, or an empty string. */
  String typeParameters(List<? extends TypeParameterElement> parameters) {
    if (parameters.isEmpty()) {
      return "";
    }
    StringBuilder out = new StringBuilder("<");
    for (int index = 0; index < parameters.size(); index++) {
      TypeParameterElement parameter = parameters.get(index);
      if (index > 0) {
        out.append(", ");
      }
      out.append(parameter.getSimpleName());
      List<String> bounds = new ArrayList<>();
      for (TypeMirror bound : parameter.getBounds()) {
        String printed = type(bound);
        if (!"Object".equals(printed)) {
          bounds.add(printed);
        }
      }
      if (!bounds.isEmpty()) {
        out.append(" extends ").append(String.join(" & ", bounds));
      }
    }
    return out.append('>').toString();
  }

  /** The erased simple name of a type, with arrays written {@code T[]}. */
  String erasedSimple(TypeMirror mirror) {
    TypeMirror erased = types.erasure(mirror);
    if (erased.getKind() == TypeKind.ARRAY) {
      return erasedSimple(((ArrayType) erased).getComponentType()) + "[]";
    }
    if (erased.getKind() == TypeKind.DECLARED) {
      return ((DeclaredType) erased).asElement().getSimpleName().toString();
    }
    return type(erased);
  }

  /** The erased qualified name of a type, as used by {@code implements}. */
  String erasedQualified(TypeMirror mirror) {
    TypeMirror erased = types.erasure(mirror);
    if (erased.getKind() == TypeKind.DECLARED) {
      return ((TypeElement) ((DeclaredType) erased).asElement()).getQualifiedName().toString();
    }
    return erased.toString();
  }

  /** The overload list of a constructor or method. */
  List<String> overload(ExecutableElement executable) {
    List<String> out = new ArrayList<>();
    for (VariableElement parameter : executable.getParameters()) {
      out.add(erasedSimple(parameter.asType()));
    }
    return out;
  }

  /** The qualified name of a symbol, without any overload list. */
  static String qualified(Element element) {
    if (element instanceof TypeElement) {
      return ((TypeElement) element).getQualifiedName().toString();
    }
    if (element instanceof PackageElement) {
      return ((PackageElement) element).getQualifiedName().toString();
    }
    Element owner = element.getEnclosingElement();
    String name =
        element.getKind() == ElementKind.CONSTRUCTOR
            ? owner.getSimpleName().toString()
            : element.getSimpleName().toString();
    return qualified(owner) + "." + name;
  }

  /** The reader-facing key of a symbol: {@code a.B.m(int,String)}. */
  String key(Element element) {
    if (element instanceof ExecutableElement) {
      return qualified(element)
          + "("
          + String.join(",", overload((ExecutableElement) element))
          + ")";
    }
    return qualified(element);
  }
}
